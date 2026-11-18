; DELİL — Windows 11 x64 kurulum betiği (Inno Setup 6+)
;
; Üretim (önerilen):
;   scripts/package-windows.sh
;
; Betik, packaging/windows/build-config.iss dosyasını üretir ve bu dosya
; MyAppVersion ile MyDistDir değerlerini tanımlar. Böylece komut satırındaki
; /D... anahtarlarına gerek kalmaz (Git Bash/MSYS bunları bozabiliyor).
;
; Doğrudan derleme (Windows'ta):
;   iscc /DMyAppVersion=0.1.0 /DMyDistDir=..\..\dist packaging\windows\delil.iss
;
; Kurulum paketi Whisper ve OCR model dosyalarını İÇERİR; kullanıcıdan ayrıca
; indirme veya teknik bağımlılık kurması istenmez.

; scripts/package-windows.sh tarafından üretilen yapılandırma (varsa) ÖNCE
; okunur; böylece sürüm ve dizin değerleri komut satırı anahtarı olmadan gelir.
#if FileExists(AddBackslash(SourcePath) + "build-config.iss")
  #include "build-config.iss"
#endif

; Sürüm ve dağıtım dizini: yapılandırma yoksa varsayılanlar kullanılır.
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#ifndef MyDistDir
  #define MyDistDir "..\..\dist"
#endif

[Setup]
; AppId, Windows yükleme kaydı için kalıcı benzersiz GUID'dir.
; Sürümler arası sabit kalmalıdır (yükseltmelerin tanınması için).
; Değer `uuidgen`/`uuid.uuid5` ile üretildi ve bu ürüne sabitlendi.
AppId={{1767A283-CC0D-5120-A7DB-033D8319E4E3}
AppName=DELİL
AppVersion={#MyAppVersion}
AppVerName=DELİL {#MyAppVersion}
AppPublisher=DELİL
AppPublisherURL=https://github.com/azmisahin-gov/delil
DefaultDirName={autopf}\DELİL
DefaultGroupName=DELİL
DisableProgramGroupPage=yes
OutputDir={#MyDistDir}
OutputBaseFilename=DELIL-Setup-{#MyAppVersion}-x64
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.22000
PrivilegesRequired=admin
WizardStyle=modern
; Kod imzalama sertifikası sağlanırsa aşağıdaki satırlar etkinleştirilir.
; SignTool=signtool
; SignedUninstaller=yes

[Languages]
Name: "turkish"; MessagesFile: "compiler:Languages\Turkish.isl"

[Tasks]
Name: "desktopicon"; Description: "Masaüstü kısayolu oluştur"; GroupDescription: "Kısayollar:"

[Files]
Source: "{#MyDistDir}\delil.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyDistDir}\delil-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyDistDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyDistDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#MyDistDir}\docs\*"; DestDir: "{app}\docs"; Flags: ignoreversion recursesubdirs
Source: "{#MyDistDir}\models\*"; DestDir: "{app}\models"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\DELİL"; Filename: "{app}\delil.exe"
Name: "{group}\DELİL Komut Satırı"; Filename: "{app}\delil-cli.exe"
Name: "{group}\Kullanıcı Kılavuzu"; Filename: "{app}\docs\KULLANICI_KILAVUZU.md"
Name: "{autodesktop}\DELİL"; Filename: "{app}\delil.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\delil.exe"; Description: "DELİL'i başlat"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Vaka deposu kaldırma sırasında SİLİNMEZ; kullanıcı verisi korunur.
; Yalnızca uygulama dosyaları kaldırılır.
Type: filesandordirs; Name: "{app}\models"

[Code]
// Disk alanı kontrolü: model ve çalışma alanı için yeterli boş alan.
function InitializeSetup(): Boolean;
var
  FreeMB: Cardinal;
  TotalMB: Cardinal;
begin
  Result := True;
  if GetSpaceOnDisk(ExpandConstant('{autopf}'), True, FreeMB, TotalMB) then
  begin
    if FreeMB < 4096 then
    begin
      if MsgBox('DELİL için en az 4 GB boş disk alanı önerilir. Devam edilsin mi?',
                mbConfirmation, MB_YESNO) = IDNO then
        Result := False;
    end;
  end;
end;
