#define CandidateName "PTTStella Slint Candidate"
#define CandidatePath "{localappdata}\Programs\PTTStella Slint Candidate"

[Setup]
AppId=io.github.yam276.pttstella.slint.candidate
AppName={#CandidateName}
AppVersion={#AppVersion}
AppPublisher=yam276
AppPublisherURL=https://github.com/yam276/PTTStella
DefaultDirName={#CandidatePath}
UsePreviousAppDir=no
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible and not arm64
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
OutputDir={#OutputDir}
OutputBaseFilename={#OutputName}
SetupIconFile={#SetupIcon}
UninstallDisplayIcon={app}\pttstella-slint.exe
VersionInfoVersion={#NumericVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
SetupMutex=PTTStellaSlintCandidateSetup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesetraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"

[CustomMessages]
english.CandidateDirectory=Use the default location for PTTStella Slint Candidate.
chinesetraditional.CandidateDirectory=請使用 PTTStella Slint 候選版的預設安裝位置。

[Files]
Source: "{#PackageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\{#CandidateName}"; Filename: "{app}\pttstella-slint.exe"

[Code]
function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if CompareText(RemoveBackslashUnlessRoot(WizardDirValue),
      ExpandConstant('{#CandidatePath}')) <> 0 then
    Result := CustomMessage('CandidateDirectory');
end;
