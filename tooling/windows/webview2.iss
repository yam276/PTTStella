; Shared prerequisite policy. The host supplies registry and process operations.
function RuntimeAt(const RootKey: HKEY): Boolean;
var
  Value: String;
  Version: Int64;
begin
  Value := '';
  Version := 0;
  Result := ReadWebViewVersion(RootKey, Value) and
    StrToVersion(Trim(Value), Version) and (Version > 0);
end;

function HasWebViewRuntime: Boolean;
begin
  Result := RuntimeAt(HKLM32) or RuntimeAt(HKCU32);
  if not Result and IsWin64 then
    Result := RuntimeAt(HKLM64) or RuntimeAt(HKCU64);
end;

function EnsureWebViewRuntime(var NeedsRestart: Boolean): String;
var
  ResultCode: Integer;
begin
  Result := '';
  if HasWebViewRuntime then
  begin
    Log('WebView2 Runtime is already installed; skipping bootstrapper.');
    Exit;
  end;
  ResultCode := 0;
  try
    if not RunWebViewBootstrapper(ResultCode) then
    begin
      Result := FmtMessage(CustomMessage('WebViewStartFailed'), [IntToStr(ResultCode)]);
      Exit;
    end;
  except
    Result := FmtMessage(CustomMessage('WebViewPrepareFailed'), [GetExceptionMessage]);
    Exit;
  end;
  Log(Format('WebView2 bootstrapper exit code: %d', [ResultCode]));
  { Registry presence is the postcondition, including a concurrent installation. }
  if HasWebViewRuntime then
    Exit;
  if ResultCode = 3010 then
  begin
    NeedsRestart := True;
    Result := CustomMessage('WebViewRestart');
  end
  else
    Result := FmtMessage(CustomMessage('WebViewInstallFailed'), [IntToStr(ResultCode)]);
end;
