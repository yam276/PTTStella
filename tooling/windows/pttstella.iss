; Slint desktop entry. Same AppId and folder as earlier PTTStella releases, so
; Setup upgrades an existing installation in place. No WebView2 prerequisite.
#ifndef AppVersion
  #error AppVersion is required
#endif

[Setup]
AppId=io.github.yam276.pttstella
AppName=PTTStella
AppVersion={#AppVersion}
AppPublisher=yam276
AppPublisherURL=https://github.com/yam276/PTTStella
DefaultDirName={localappdata}\Programs\PTTStella
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible and not arm64
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
SetupIconFile={#PackageDir}\PTTStella.ico
UninstallDisplayIcon={app}\pttstella.exe
VersionInfoVersion={#NumericVersion}
LicenseFile={#PackageDir}\licenses\PTTStella-GPL-3.0-or-later.txt
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
CloseApplications=yes
RestartApplications=no
SetupMutex=PTTStellaSetup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesetraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"

[InstallDelete]
; Notices of the previous frontend are replaced as a whole.
Type: filesandordirs; Name: "{app}\licenses"

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\PTTStella"; Filename: "{app}\pttstella.exe"
