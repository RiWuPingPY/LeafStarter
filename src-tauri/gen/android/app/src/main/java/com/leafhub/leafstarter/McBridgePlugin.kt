package com.leafhub.mcsupply

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.Settings
import androidx.activity.result.ActivityResult
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class PackagesArgs {
    var packages: Array<String> = arrayOf()
}

@InvokeArg
class PickArgs {
    var initialUri: String? = null
}

@InvokeArg
class UriArgs {
    var uri: String = ""
}

@InvokeArg
class ListArgs {
    var uri: String = ""
    var path: String = ""
}

@InvokeArg
class CopyArgs {
    var uri: String = ""
    var rel: String = ""
    var srcRoot: String = ""
    var files: Array<String> = arrayOf()
}

/**
 * 方块补给的安卓桥接：
 * - 查询装了哪些启动器（包名在 Rust 那边列好）
 * - "所有文件访问"权限（写 FCL 之类放在公共存储里的目录）
 * - 通过系统文件选择器授权启动器自己暴露的目录（ZL2 / Pojav 的 Android/data 私有目录）
 * - 把下载好的文件批量写进授权目录
 */
@TauriPlugin
class McBridgePlugin(private val activity: Activity) : Plugin(activity) {
    private data class Child(val docId: String, val isDir: Boolean, val size: Long)

    private val resolver get() = activity.contentResolver

    /** 目录 docId → 子项（按名字），一次复制过程中复用，避免每个文件都去查一遍 */
    private val childCache = HashMap<String, MutableMap<String, Child>>()

    /** Tauri 在主线程上调插件命令；读写文件都放到这个后台线程，免得界面卡死。单线程也保证了 childCache 不会并发访问 */
    private val io = java.util.concurrent.Executors.newSingleThreadExecutor()

    @Command
    fun installedApps(invoke: Invoke) {
        val args = invoke.parseArgs(PackagesArgs::class.java)
        val pm = activity.packageManager
        val apps = JSArray()
        for (p in args.packages) {
            try {
                val info = pm.getPackageInfo(p, 0)
                val label = info.applicationInfo?.loadLabel(pm)?.toString() ?: p
                apps.put(JSObject().apply {
                    put("package", p)
                    put("label", label)
                    put("version", info.versionName ?: "")
                })
            } catch (_: PackageManager.NameNotFoundException) {
            }
        }
        invoke.resolve(JSObject().apply { put("apps", apps) })
    }

    private fun hasAllFiles(): Boolean =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            Environment.isExternalStorageManager()
        } else {
            ContextCompat.checkSelfPermission(activity, Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED
        }

    @Command
    fun allFilesAccess(invoke: Invoke) {
        invoke.resolve(JSObject().apply { put("granted", hasAllFiles()) })
    }

    @Command
    fun requestAllFilesAccess(invoke: Invoke) {
        activity.runOnUiThread {
            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    try {
                        activity.startActivity(
                            Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:" + activity.packageName))
                        )
                    } catch (_: Exception) {
                        activity.startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
                    }
                } else {
                    ActivityCompat.requestPermissions(
                        activity,
                        arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE, Manifest.permission.WRITE_EXTERNAL_STORAGE),
                        4071
                    )
                }
                invoke.resolve()
            } catch (e: Exception) {
                invoke.reject("打不开权限设置：${e.message}")
            }
        }
    }

    @Command
    fun pickTree(invoke: Invoke) {
        val args = invoke.parseArgs(PickArgs::class.java)
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            addFlags(
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                    Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION or Intent.FLAG_GRANT_PREFIX_URI_PERMISSION
            )
            val initial = args.initialUri
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !initial.isNullOrEmpty()) {
                putExtra(DocumentsContract.EXTRA_INITIAL_URI, Uri.parse(initial))
            }
        }
        activity.runOnUiThread { startActivityForResult(invoke, intent, "pickTreeResult") }
    }

    @ActivityCallback
    fun pickTreeResult(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.reject("没有选择目录")
            return
        }
        try {
            resolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        } catch (e: SecurityException) {
            invoke.reject("无法保存授权：${e.message}")
            return
        }
        io.execute {
            try {
                childCache.clear()
                val root = DocumentsContract.getTreeDocumentId(uri)
                val rel = findMinecraft(uri, root)
                invoke.resolve(JSObject().apply {
                    put("uri", uri.toString())
                    put("rel", rel ?: "")
                    put("found", rel != null)
                    put("label", displayName(uri, root) ?: uri.lastPathSegment ?: "")
                })
            } catch (e: Exception) {
                invoke.reject("读取授权目录失败：${e.message}")
            }
        }
    }

    @Command
    fun releaseTree(invoke: Invoke) {
        val args = invoke.parseArgs(UriArgs::class.java)
        try {
            resolver.releasePersistableUriPermission(
                Uri.parse(args.uri),
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
            )
        } catch (_: Exception) {
        }
        invoke.resolve()
    }

    /** 授权目录里某个子目录下的文件夹名（用来列出已有版本） */
    @Command
    fun listDirs(invoke: Invoke) {
        val args = invoke.parseArgs(ListArgs::class.java)
        io.execute { listDirsNow(invoke, args) }
    }

    private fun listDirsNow(invoke: Invoke, args: ListArgs) {
        try {
            childCache.clear()
            val tree = Uri.parse(args.uri)
            var doc = DocumentsContract.getTreeDocumentId(tree)
            for (seg in args.path.split('/').filter { it.isNotEmpty() }) {
                val c = children(tree, doc)[seg]
                if (c == null || !c.isDir) {
                    invoke.resolve(JSObject().apply { put("names", JSArray()) })
                    return
                }
                doc = c.docId
            }
            val names = JSArray()
            children(tree, doc).filter { it.value.isDir }.keys.sorted().forEach { names.put(it) }
            invoke.resolve(JSObject().apply { put("names", names) })
        } catch (e: Exception) {
            invoke.resolve(JSObject().apply { put("names", JSArray()); put("error", e.message ?: e.toString()) })
        }
    }

    /** 把 srcRoot 下的一批文件（相对路径）复制到授权目录的 rel 子目录里；大小相同的已存在文件跳过 */
    @Command
    fun copyIn(invoke: Invoke) {
        val args = invoke.parseArgs(CopyArgs::class.java)
        io.execute { copyInNow(invoke, args) }
    }

    private fun copyInNow(invoke: Invoke, args: CopyArgs) {
        var copied = 0
        var skipped = 0
        try {
            val tree = Uri.parse(args.uri)
            var base = DocumentsContract.getTreeDocumentId(tree)
            for (seg in args.rel.split('/').filter { it.isNotEmpty() }) base = ensureDir(tree, base, seg)
            val buf = ByteArray(256 * 1024)
            for (rel in args.files) {
                val src = File(args.srcRoot, rel)
                if (!src.isFile) continue
                val segs = rel.split('/').filter { it.isNotEmpty() }
                var dir = base
                for (s in segs.dropLast(1)) dir = ensureDir(tree, dir, s)
                val name = segs.last()
                val existing = children(tree, dir)[name]
                if (existing != null && !existing.isDir && existing.size == src.length()) {
                    skipped++
                    continue
                }
                val docId = existing?.docId ?: createFile(tree, dir, name)
                val docUri = DocumentsContract.buildDocumentUriUsingTree(tree, docId)
                resolver.openOutputStream(docUri, "wt").use { out ->
                    if (out == null) throw IllegalStateException("无法写入 $rel")
                    src.inputStream().use { input ->
                        while (true) {
                            val n = input.read(buf)
                            if (n <= 0) break
                            out.write(buf, 0, n)
                        }
                    }
                }
                children(tree, dir)[name] = Child(docId, false, src.length())
                copied++
            }
            invoke.resolve(JSObject().apply { put("copied", copied); put("skipped", skipped) })
        } catch (e: Exception) {
            invoke.resolve(JSObject().apply {
                put("copied", copied)
                put("skipped", skipped)
                put("error", e.message ?: e.toString())
            })
        }
    }

    // ---------- SAF 小工具 ----------

    private fun children(tree: Uri, docId: String): MutableMap<String, Child> {
        childCache[docId]?.let { return it }
        val map = HashMap<String, Child>()
        val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, docId)
        resolver.query(
            uri,
            arrayOf(
                DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                DocumentsContract.Document.COLUMN_MIME_TYPE,
                DocumentsContract.Document.COLUMN_SIZE
            ),
            null, null, null
        )?.use { c ->
            while (c.moveToNext()) {
                val id = c.getString(0) ?: continue
                val name = c.getString(1) ?: continue
                val isDir = c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR
                val size = if (c.isNull(3)) -1L else c.getLong(3)
                map[name] = Child(id, isDir, size)
            }
        }
        childCache[docId] = map
        return map
    }

    private fun ensureDir(tree: Uri, parent: String, name: String): String {
        val existing = children(tree, parent)[name]
        if (existing != null && existing.isDir) return existing.docId
        val parentUri = DocumentsContract.buildDocumentUriUsingTree(tree, parent)
        val created = DocumentsContract.createDocument(resolver, parentUri, DocumentsContract.Document.MIME_TYPE_DIR, name)
            ?: throw IllegalStateException("无法创建文件夹 $name")
        val id = DocumentsContract.getDocumentId(created)
        children(tree, parent)[name] = Child(id, true, 0)
        childCache[id] = HashMap()
        return id
    }

    private fun createFile(tree: Uri, parent: String, name: String): String {
        val parentUri = DocumentsContract.buildDocumentUriUsingTree(tree, parent)
        val created = DocumentsContract.createDocument(resolver, parentUri, "application/octet-stream", name)
            ?: throw IllegalStateException("无法创建文件 $name")
        return DocumentsContract.getDocumentId(created)
    }

    private fun displayName(tree: Uri, docId: String): String? {
        val uri = DocumentsContract.buildDocumentUriUsingTree(tree, docId)
        return resolver.query(uri, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }
    }

    /**
     * 在授权的目录树里找 .minecraft：树根本身有 versions 就是它，否则往下最多找 4 层。
     * ZL2 / Pojav 的提供器根目录下是 android_data/files/.minecraft。
     */
    private fun findMinecraft(tree: Uri, root: String): String? {
        val skip = setOf("assets", "libraries", "saves", "mods", "resourcepacks", "shaderpacks", "logs", "cache")
        if (children(tree, root)["versions"]?.isDir == true) return ""
        val queue = ArrayDeque<Pair<String, String>>()
        queue.add(root to "")
        var visited = 0
        while (queue.isNotEmpty() && visited < 400) {
            val (doc, path) = queue.removeFirst()
            visited++
            val depth = if (path.isEmpty()) 0 else path.count { it == '/' } + 1
            for ((name, c) in children(tree, doc)) {
                if (!c.isDir) continue
                val childPath = if (path.isEmpty()) name else "$path/$name"
                if (name == ".minecraft") return childPath
                if (depth < 3 && name !in skip) queue.add(c.docId to childPath)
            }
        }
        return null
    }
}
