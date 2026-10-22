[Setup]
AppName=PTTStella installer policy probe
AppVersion=1.0.0
DefaultDirName={tmp}\PTTStellaPolicyProbe
CreateAppDir=no
Uninstallable=no
PrivilegesRequired=lowest
SetupArchitecture=x64
ArchitecturesAllowed=x64compatible and not arm64
OutputDir={#OutputDir}
OutputBaseFilename=installer-policy-probe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl,{#SourcePath}\installer.en.isl"

[Code]
var
  Versions: array[0..3] of String;
  Runs, NextCode, Assertions: Integer;
  Starts, CreatesRuntime, Throws: Boolean;

function ReadWebViewVersion(const RootKey: HKEY; var Value: String): Boolean;
begin
  if RootKey = HKLM32 then Value := Versions[0]
  else if RootKey = HKCU32 then Value := Versions[1]
  else if RootKey = HKLM64 then Value := Versions[2]
  else if RootKey = HKCU64 then Value := Versions[3]
  else RaiseException('Unexpected registry root');
  Result := Value <> '';
end;

function RunWebViewBootstrapper(var ResultCode: Integer): Boolean;
begin
  Runs := Runs + 1;
  if Throws then RaiseException('synthetic bootstrapper failure');
  if CreatesRuntime then Versions[3] := '155.0.4283.32';
  ResultCode := NextCode;
  Result := Starts;
end;

#include "webview2.iss"

procedure AssertState(Condition: Boolean; const Description: String);
begin
  Assertions := Assertions + 1;
  if not Condition then RaiseException(Description);
end;

procedure ResetProbe;
var I: Integer;
begin
  for I := 0 to 3 do Versions[I] := '';
  Runs := 0;
  NextCode := 0;
  Starts := True;
  CreatesRuntime := False;
  Throws := False;
end;

function InitializeSetup: Boolean;
var
  I: Integer;
  NeedsRestart: Boolean;
  Error, Report: String;
begin
  Assertions := 0;
  for I := 0 to 3 do
  begin
    ResetProbe;
    Versions[I] := '155.0.4283.32';
    NeedsRestart := False;
    Error := EnsureWebViewRuntime(NeedsRestart);
    AssertState((Error = '') and (Runs = 0) and not NeedsRestart,
      'An installed runtime must skip installation in each registry view');
  end;

  ResetProbe;
  Versions[0] := '0.0.0.0';
  Versions[1] := 'invalid';
  Versions[2] := '  ';
  CreatesRuntime := True;
  NeedsRestart := False;
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error = '') and (Runs = 1), 'Missing/invalid versions must install exactly once');
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error = '') and (Runs = 1), 'Post-install retry must detect the installed runtime');

  ResetProbe;
  Starts := False;
  NextCode := 5;
  NeedsRestart := False;
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error <> '') and (Runs = 1) and not NeedsRestart, 'Launch failure must stop Setup');

  ResetProbe;
  NextCode := 1;
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error <> '') and (Runs = 1), 'Cancelled/failed bootstrapper must stop Setup');

  ResetProbe;
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error <> '') and (Runs = 1), 'Exit zero without an installed runtime must stop Setup');

  ResetProbe;
  NextCode := 3010;
  NeedsRestart := False;
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error <> '') and NeedsRestart, 'Restart-required result must stop before copying files');

  ResetProbe;
  Throws := True;
  NeedsRestart := False;
  Error := EnsureWebViewRuntime(NeedsRestart);
  AssertState((Error <> '') and (Runs = 1), 'Extraction/integrity failure must stop Setup');

  Report := ExpandConstant('{param:Report|}');
  if Report = '' then RaiseException('Probe report path is required');
  if not SaveStringToFile(Report,
    '{"status":"passed","assertions":' + IntToStr(Assertions) +
    ',"real_registry_access":false,"real_bootstrapper_executed":false}', False) then
    RaiseException('Cannot save probe report');
  Result := True;
end;
