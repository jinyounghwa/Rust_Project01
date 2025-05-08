@echo off
echo 네트워크 모니터 패키징 시작...

set PACKAGE_DIR=package
set RELEASE_DIR=target\release

rem 패키지 디렉토리 생성
if exist %PACKAGE_DIR% rmdir /s /q %PACKAGE_DIR%
mkdir %PACKAGE_DIR%

rem 실행 파일 복사
copy %RELEASE_DIR%\network_monitor.exe %PACKAGE_DIR%\
copy %RELEASE_DIR%\network_monitor_gui.exe %PACKAGE_DIR%\Network_Monitor_GUI.exe

rem 설정 파일 복사
if exist config.toml copy config.toml %PACKAGE_DIR%\

rem 바로가기 생성을 위한 VBS 스크립트 생성
echo Set oWS = WScript.CreateObject("WScript.Shell") > %PACKAGE_DIR%\create_shortcut.vbs
echo sLinkFile = "%USERPROFILE%\Desktop\Network Monitor GUI.lnk" >> %PACKAGE_DIR%\create_shortcut.vbs
echo Set oLink = oWS.CreateShortcut(sLinkFile) >> %PACKAGE_DIR%\create_shortcut.vbs
echo oLink.TargetPath = "%CD%\%PACKAGE_DIR%\Network_Monitor_GUI.exe" >> %PACKAGE_DIR%\create_shortcut.vbs
echo oLink.WorkingDirectory = "%CD%\%PACKAGE_DIR%" >> %PACKAGE_DIR%\create_shortcut.vbs
echo oLink.Description = "네트워크 모니터 GUI" >> %PACKAGE_DIR%\create_shortcut.vbs
echo oLink.IconLocation = "%CD%\%PACKAGE_DIR%\Network_Monitor_GUI.exe, 0" >> %PACKAGE_DIR%\create_shortcut.vbs
echo oLink.Save >> %PACKAGE_DIR%\create_shortcut.vbs

rem README 파일 생성
echo 네트워크 모니터 GUI > %PACKAGE_DIR%\README.txt
echo =================== >> %PACKAGE_DIR%\README.txt
echo. >> %PACKAGE_DIR%\README.txt
echo 설치 방법: >> %PACKAGE_DIR%\README.txt
echo 1. 이 폴더의 내용을 원하는 위치에 복사합니다. >> %PACKAGE_DIR%\README.txt
echo 2. create_shortcut.vbs를 실행하여 바탕화면에 바로가기를 만듭니다. >> %PACKAGE_DIR%\README.txt
echo. >> %PACKAGE_DIR%\README.txt
echo 사용 방법: >> %PACKAGE_DIR%\README.txt
echo - Network_Monitor_GUI.exe를 실행하여 GUI 모드로 시작합니다. >> %PACKAGE_DIR%\README.txt
echo - 설정은 config.toml 파일에서 수정할 수 있습니다. >> %PACKAGE_DIR%\README.txt

echo 패키징 완료! %PACKAGE_DIR% 디렉토리에 파일이 생성되었습니다.
