; 安装程序的补充步骤（tauri.conf.json → bundle.windows.nsis.installerHooks）
; - 注册 leafstarter:// 链接：朋友发来的邀请链接点一下就打开启动器、弹出「用邀请码加入」
; - 卸载时删掉链接注册和自动更新留下的临时文件（.new / .old / .download）

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\leafstarter" "" "URL:LeafStarter"
  WriteRegStr HKCU "Software\Classes\leafstarter" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\leafstarter\DefaultIcon" "" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr HKCU "Software\Classes\leafstarter\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\leafstarter"
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.new"
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.old"
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.download"
  Delete "$INSTDIR\${MAINBINARYNAME}.exe.download.part"
!macroend
