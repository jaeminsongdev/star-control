#ifndef AppVersion
  #error AppVersion must be supplied by build-installer.ps1
#endif
#ifndef TargetArch
  #error TargetArch must be x64 or arm64
#endif
#ifndef StageDir
  #error StageDir must point to a verified release stage
#endif
#ifndef OutputDir
  #error OutputDir must point to the dist directory
#endif

#define AppGuid "F067E3E8-4B01-4F98-9FA5-634283A00D2C"

[Setup]
AppId={{{#AppGuid}}
AppName=Star-Control
AppVersion={#AppVersion}
AppPublisher=Star-Control contributors
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\Star-Control
UsePreviousAppDir=yes
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename=star-control-windows-{#TargetArch}-{#AppVersion}-setup
Compression=lzma2/max
SolidCompression=yes
Uninstallable=yes
UninstallDisplayName=Star-Control
UninstallDisplayIcon={app}\star.exe
SetupLogging=yes
ChangesEnvironment=no
CloseApplications=no
RestartApplications=no
RestartIfNeededByRun=no
WizardStyle=modern

#if TargetArch == "x64"
ArchitecturesAllowed=x64compatible and not arm64
ArchitecturesInstallIn64BitMode=x64compatible
#elif TargetArch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
  #error TargetArch must be x64 or arm64
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[CustomMessages]
english.CodexNoticeCaption=Codex integration
english.CodexNoticeDescription=Review the required follow-up after installation.
english.CodexNoticeMessage=Star-Control uses an installed local Marketplace. After setup, run star integration status. If manual_commands are present, complete them in the official Codex CLI or Plugin screen, open a new task, and review and trust the SessionStart Hook. Setup never edits Codex config, cache, or Hook trust files directly.
english.OfflineInstallRequired=Close Codex and all Star-Control processes, then retry from a separate PowerShell. Setup stopped before changing any files.
korean.CodexNoticeCaption=Codex 연동
korean.CodexNoticeDescription=설치 뒤 필요한 후속 조치를 확인하세요.
korean.CodexNoticeMessage=Star-Control은 설치된 로컬 Marketplace를 사용합니다. 설치 뒤 star integration status를 실행하세요. manual_commands가 있으면 공식 Codex CLI나 Plugin 화면에서 완료하고 새 작업을 연 뒤 SessionStart Hook을 검토하고 신뢰해야 합니다. 설치 파일은 Codex 설정, cache 또는 Hook 신뢰 파일을 직접 수정하지 않습니다.
korean.OfflineInstallRequired=Codex 앱과 모든 Star-Control 프로세스를 종료한 뒤 별도 PowerShell에서 다시 실행하세요. 설치는 파일을 변경하기 전에 중단되었습니다.

[Tasks]
Name: "codexintegration"; Description: "Codex Plugin, MCP, Hook 연동 구성"
Name: "autostart"; Description: "현재 사용자 로그인 시 Star-Control Controller 자동 시작"

[Files]
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[UninstallRun]
Filename: "{app}\star.exe"; Parameters: "integration uninstall --json"; Flags: runhidden waituntilterminated; RunOnceId: "StarControlCodexIntegrationRemove"
Filename: "{app}\star.exe"; Parameters: "controller autostart disable"; Flags: runhidden waituntilterminated; RunOnceId: "StarControlAutostartRemove"

[UninstallDelete]
Type: files; Name: "{app}\star-control-install.v1.json"
Type: files; Name: "{localappdata}\Star-Control\installation\installation-record.v1.json"
Type: dirifempty; Name: "{localappdata}\Star-Control\installation"
Type: dirifempty; Name: "{localappdata}\Star-Control"
Type: filesandordirs; Name: "{userappdata}\Star-Control"; Check: PurgeDataRequested
Type: filesandordirs; Name: "{localappdata}\Star-Control"; Check: PurgeDataRequested

[Code]
var
  PostInstallFailed: Boolean;
  CodexNoticePage: TOutputMsgWizardPage;

procedure InitializeWizard;
begin
  CodexNoticePage := CreateOutputMsgPage(
    wpSelectTasks,
    ExpandConstant('{cm:CodexNoticeCaption}'),
    ExpandConstant('{cm:CodexNoticeDescription}'),
    ExpandConstant('{cm:CodexNoticeMessage}')
  );
end;

function PurgeDataRequested: Boolean;
var
  Index: Integer;
begin
  Result := False;
  for Index := 1 to ParamCount do
  begin
    if Uppercase(ParamStr(Index)) = '/PURGEDATA' then
    begin
      Result := True;
      exit;
    end;
  end;
end;

function OfflineProcessesAreRunning: Boolean;
var
  Locator, Services, Processes: Variant;
begin
  Result := True;
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Services := Locator.ConnectServer('.', 'root\CIMV2');
    Processes := Services.ExecQuery(
      'SELECT ProcessId FROM Win32_Process WHERE ' +
      'Name = ''ChatGPT.exe'' OR ' +
      'Name = ''star-controller.exe'' OR ' +
      'Name = ''star-mcp.exe'''
    );
    Result := Processes.Count > 0;
  except
    Log('Offline process preflight failed; setup remains fail-closed.');
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if OfflineProcessesAreRunning then
  begin
    Log('Codex or Star-Control is active; setup stopped before file installation.');
    Result := ExpandConstant('{cm:OfflineInstallRequired}');
  end;
end;

function RunRequired(const FileName, Parameters, FailureMessage: String): Boolean;
var
  ResultCode: Integer;
begin
  Result := Exec(FileName, Parameters, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and
            (ResultCode = 0);
  if not Result then
  begin
    PostInstallFailed := True;
    Log(FailureMessage + ' (exit=' + IntToStr(ResultCode) + ')');
    SuppressibleMsgBox(
      FailureMessage + #13#10 + 'exit=' + IntToStr(ResultCode),
      mbError,
      MB_OK,
      IDOK
    );
  end;
end;

procedure RunOptional(const FileName, Parameters, FailureMessage: String);
var
  ResultCode: Integer;
begin
  if not Exec(FileName, Parameters, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) or
     (ResultCode <> 0) then
    Log(FailureMessage + ' (exit=' + IntToStr(ResultCode) + ')');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if not RunRequired(
      ExpandConstant('{app}\star.exe'),
      'installation finalize --architecture {#TargetArch} --replace-existing --json',
      'Star-Control installation manifest finalization failed'
    ) then
      exit;
    if not RunRequired(
      ExpandConstant('{app}\star.exe'),
      'installation bridge initialize --state-generation bootstrap_v2 --json',
      'Star-Control Bootstrap Bridge v2 initialization failed'
    ) then
      exit;
    if WizardIsTaskSelected('codexintegration') then
    begin
      if not RunRequired(
        ExpandConstant('{app}\star.exe'),
        'integration install --json',
        'Star-Control Codex integration rendering failed'
      ) then
        exit;
    end
    else
    begin
      RunOptional(
        ExpandConstant('{app}\star.exe'),
        'integration uninstall --json',
        'Star-Control Codex integration needs manual deregistration'
      );
    end;
    if WizardIsTaskSelected('autostart') then
      RunRequired(
        ExpandConstant('{app}\star.exe'),
        'controller autostart enable',
        'Star-Control current-user autostart registration failed'
      )
    else
      RunRequired(
        ExpandConstant('{app}\star.exe'),
        'controller autostart disable',
        'Star-Control current-user autostart removal failed'
      );
  end;
end;

function GetCustomSetupExitCode: Integer;
begin
  if PostInstallFailed then
    Result := 101
  else
    Result := 0;
end;
