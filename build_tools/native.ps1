#requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Build', 'Check', 'Package', 'Inspect', 'Install', 'Remove')][string]$Action,
    [ValidateSet('debug', 'release')][string]$Configuration = 'release',
    [ValidateSet('default', 'minimal')][string]$Features = 'default',
    [string]$OutputDirectory,
    [string]$Archive,
    [string]$Destination
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
if (-not $IsWindows -or [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') { throw 'This command requires native x64 Windows, not a guest/cross build' }
$root = Split-Path $PSScriptRoot -Parent
$target = 'x86_64-pc-windows-msvc'
$utf8 = [Text.UTF8Encoding]::new($false)
function Invoke-Tool([string]$File, [string[]]$Arguments) {
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File exited with code $LASTEXITCODE" }
}
function Get-ToolText([string]$File, [string[]]$Arguments) {
    $text = & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File exited with code $LASTEXITCODE" }
    return ($text -join "`n")
}
function Invoke-NativeSuite([string]$Suite, [string[]]$Arguments) {
    $start = [Diagnostics.ProcessStartInfo]::new('cargo')
    $start.WorkingDirectory = $root
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $start.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    $started = $false
    try {
        if (-not $process.Start()) { throw "Cargo did not start for $Suite" }
        $started = $true
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(3600000)) { throw "Native suite timed out: $Suite" }
        if (-not $stdout.Wait(30000) -or -not $stderr.Wait(30000)) { throw "Native suite streams did not close: $Suite" }
        $text = $stdout.Result + "`n" + $stderr.Result
        $results = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR, $root) } else { Join-Path $root 'target' }
        $results = Join-Path $results "n10-check-results/$Configuration-$Features"
        [IO.Directory]::CreateDirectory($results) | Out-Null
        [IO.File]::WriteAllText((Join-Path $results "$Suite.log"), $text, $utf8)
        Write-Host $text
        if ($process.ExitCode -ne 0) { throw "cargo exited with code $($process.ExitCode) for $Suite" }
        return $text
    } finally {
        if ($started -and -not $process.HasExited) { $process.Kill($true); $process.WaitForExit() }
        $process.Dispose()
    }
}
function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, ($Value | ConvertTo-Json -Depth 30) + "`n", $utf8)
}
function Build-NativeHelp([string]$Help, [string]$Doctrees, [string]$Version) {
    Get-Command sphinx-build -ErrorAction Stop | Out-Null
    $previousDocVersion = $env:FISH_DOC_VERSION
    try {
        $env:FISH_DOC_VERSION = $Version
        Invoke-Tool 'sphinx-build' @('-j', 'auto', '-q', '-b', 'text', '-c', (Join-Path $root 'doc_src'), '-d', $Doctrees, (Join-Path $root 'doc_src'), $Help)
    } finally { $env:FISH_DOC_VERSION = $previousDocVersion }
    [IO.File]::WriteAllText((Join-Path $Help 'fish-help-version'), $Version + "`n", $utf8)
}
function Get-Files([string]$Directory) {
    $files = [string[]]@(Get-ChildItem -LiteralPath $Directory -File -Recurse -Force | ForEach-Object {
        [IO.Path]::GetRelativePath($Directory, $_.FullName).Replace('\', '/')
    })
    [Array]::Sort($files, [StringComparer]::Ordinal)
    return $files
}
function Assert-PlainTree([string]$Directory) {
    foreach ($item in @(Get-Item -LiteralPath $Directory) + @(Get-ChildItem -LiteralPath $Directory -Recurse -Force)) {
        if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Reparse point is not a package file: $($item.FullName)" }
    }
}
function Assert-Package([string]$Directory) {
    Assert-PlainTree $Directory
    $manifestPath = Join-Path $Directory 'package-manifest.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.schema -ne 1 -or $manifest.product -ne 'fish' -or $manifest.target -ne $target -or $manifest.signing -ne 'unsigned') {
        throw 'Not a supported unsigned x64 fish package'
    }
    $actual = @(Get-Files $Directory | Where-Object { $_ -ne 'package-manifest.json' })
    $expected = @($manifest.files | ForEach-Object { $_.path })
    if (@(Compare-Object $actual $expected -CaseSensitive).Count -ne 0) { throw 'Package has missing or untracked files' }
    $directories = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($name in $expected) {
        $parent = [IO.Path]::GetDirectoryName($name)
        while ($parent) { $null = $directories.Add($parent.Replace('\', '/')); $parent = [IO.Path]::GetDirectoryName($parent) }
    }
    foreach ($entry in Get-ChildItem -LiteralPath $Directory -Directory -Recurse -Force) {
        $relative = [IO.Path]::GetRelativePath($Directory, $entry.FullName).Replace('\', '/')
        if (-not $directories.Contains($relative)) { throw "Untracked package directory: $relative" }
    }
    foreach ($file in $manifest.files) {
        if ([IO.Path]::IsPathRooted($file.path) -or $file.path.Contains('\') -or @($file.path.Split('/') | Where-Object { $_ -in @('', '.', '..') }).Count) { throw 'Unsafe manifest path' }
        if ((Get-FileHash -LiteralPath (Join-Path $Directory $file.path) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256) {
            throw "Corrupt package file: $($file.path)"
        }
    }
    foreach ($exe in 'fish', 'fish_indent', 'fish_key_reader') {
        if ("bin/$exe.exe" -cnotin $expected) { throw "Missing real executable: $exe" }
        $info = [Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $Directory "bin/$exe.exe"))
        if ($info.ProductVersion -ne $manifest.version -or $info.FileVersion -ne $manifest.version) { throw "Executable version resource mismatch: $exe" }
    }
    return $manifest
}
function Expand-Package([string]$Zip, [string]$Directory) {
    $reader = [IO.Compression.ZipFile]::OpenRead($Zip)
    try {
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
        foreach ($entry in $reader.Entries) {
            $name = $entry.FullName
            if ([IO.Path]::IsPathRooted($name) -or $name.Contains('\') -or $name.Contains(':') -or @($name.Split('/') | Where-Object { $_ -in @('', '.', '..') }).Count -or -not $seen.Add($name)) {
                throw "Unsafe or duplicate archive entry: $name"
            }
            if (($entry.ExternalAttributes -shr 16 -band 0xF000) -eq 0xA000) { throw "Archive symlink: $name" }
            $path = Join-Path $Directory $name
            [IO.Directory]::CreateDirectory((Split-Path $path -Parent)) | Out-Null
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $path, $false)
        }
    } finally { $reader.Dispose() }
    return Assert-Package $Directory
}
function Assert-NativeHost {
    $rust = Get-ToolText 'rustc' @('-Vv')
    if ($rust -notmatch '(?m)^host: x86_64-pc-windows-msvc$') { throw 'Run with the native x64 MSVC Rust toolchain' }
    Get-Command cl.exe, link.exe, rc.exe, dumpbin.exe -ErrorAction Stop | Out-Null
    if (-not $env:WindowsSDKVersion -or [version]$env:WindowsSDKVersion.TrimEnd('\') -lt [version]'10.0.26100.0') { throw 'Initialize an x64 Windows SDK 10.0.26100.0 or newer developer environment' }
}
function Set-BuildEnvironment {
    if ($env:FISH_BUILD_DOCS -eq '0' -and $Features -eq 'default') { throw 'Default features require documentation; FISH_BUILD_DOCS=0 would silently strip help' }
    if ($env:RUSTFLAGS -and $env:RUSTFLAGS -ne '-C target-feature=+crt-static') { throw 'Use an unset RUSTFLAGS; custom flags would change the declared native distribution profile' }
    if ($env:CARGO_ENCODED_RUSTFLAGS) { throw 'CARGO_ENCODED_RUSTFLAGS would override the distribution CRT policy' }
    $env:RUSTFLAGS = '-C target-feature=+crt-static'
    $env:PCRE2_SYS_STATIC = '1'
    if ($Features -eq 'default') {
        Get-Command sphinx-build, msgfmt -ErrorAction Stop | Out-Null
        $env:FISH_BUILD_DOCS = '1'
    }
}
function Get-CargoArguments([string]$Verb) {
    $arguments = @($Verb, '--locked', '--target', $target)
    if ($Configuration -eq 'release') { $arguments += '--release' }
    if ($Features -eq 'minimal') { $arguments += '--no-default-features' }
    return $arguments
}
function Build-Native {
    Assert-NativeHost
    Set-BuildEnvironment
    Invoke-Tool 'cargo' ((Get-CargoArguments 'build') + @('--package', 'fish', '--bin', 'fish', '--bin', 'fish_indent', '--bin', 'fish_key_reader'))
}
function Inspect-Exe([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 64 -or $bytes[0] -ne 77 -or $bytes[1] -ne 90) { throw "Not a PE executable: $Path" }
    $offset = [BitConverter]::ToInt32($bytes, 60)
    if ($offset -lt 64 -or $offset + 26 -gt $bytes.Length -or [BitConverter]::ToUInt32($bytes, $offset) -ne 0x4550 -or [BitConverter]::ToUInt16($bytes, $offset + 4) -ne 0x8664) { throw "Not an AMD64 PE executable: $Path" }
    if ((Get-AuthenticodeSignature -LiteralPath $Path).Status -ne 'NotSigned') { throw "Unsigned channel contains a signed or invalid-signature executable: $Path" }
    $imports = Get-ToolText 'dumpbin.exe' @('/nologo', '/dependents', $Path)
    $dlls = @([regex]::Matches($imports, '(?im)^\s*([a-z0-9_.-]+\.dll)\s*$') | ForEach-Object { $_.Groups[1].Value.ToLowerInvariant() } | Sort-Object -Unique)
    if ($dlls.Count -eq 0) { throw "dumpbin found no imports: $Path" }
    foreach ($dll in $dlls) {
        if ($dll -notmatch '^(api-ms-win-|ext-ms-win-)' -and $dll -notin @('kernel32.dll', 'kernelbase.dll', 'ntdll.dll', 'advapi32.dll', 'bcrypt.dll', 'bcryptprimitives.dll', 'crypt32.dll', 'version.dll', 'psapi.dll', 'netapi32.dll', 'iphlpapi.dll', 'userenv.dll', 'ws2_32.dll', 'user32.dll', 'shell32.dll', 'ole32.dll', 'oleaut32.dll', 'secur32.dll', 'rpcrt4.dll', 'gdi32.dll', 'msvcrt.dll', 'ucrtbase.dll', 'synchronization.dll')) { throw "Undeployed dependency $dll imported by $Path" }
    }
    return [ordered]@{ architecture = 'AMD64'; imports = $dlls; signing = 'unsigned' }
}
Push-Location $root
try {
    switch ($Action) {
        'Build' { Build-Native }
        'Check' {
            Assert-NativeHost
            Set-BuildEnvironment
            # All integration helper binaries must match BOTH artifact profiles
            # before any dual-profile native gate starts.
            foreach ($profile in 'debug', 'release') {
                $arguments = @('build', '--locked', '--target', $target, '--package', 'fish', '--bins')
                if ($profile -eq 'release') { $arguments += '--release' }
                if ($Features -eq 'minimal') { $arguments += '--no-default-features' }
                Invoke-Tool 'cargo' $arguments
            }
            if ($Features -eq 'minimal') {
                $build = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR, $root) } else { Join-Path $root 'target' }
                $versionText = Get-ToolText (Join-Path $build "$target/$Configuration/fish.exe") @('--version')
                if ($versionText -notmatch '^fish, version (.+)$') { throw "Unexpected native check executable version: $versionText" }
                $help = Join-Path $build 'fish-docs/help'
                Build-NativeHelp $help (Join-Path $build 'fish-docs/.doctrees-text') $Matches[1]
                $env:FISH_N9_HELP_DIR = $help
            }
            Invoke-Tool 'cargo' ((Get-CargoArguments 'test') + @('--package', 'fish', '--lib'))
            Invoke-Tool 'cargo' ((Get-CargoArguments 'test') + @('--package', 'fish', '--doc'))
            foreach ($suite in 'windows_native', 'windows_n1', 'windows_n2', 'windows_n3', 'windows_n4', 'windows_n5', 'windows_n6', 'windows_n7', 'windows_n8', 'windows_n9', 'windows_n10') {
                $listing = Get-ToolText 'cargo' ((Get-CargoArguments 'test') + @('--package', 'fish', '--test', $suite, '--', '--list'))
                if ($listing -notmatch '(?m)^.+: test$') { throw "Zero-case mandatory suite: $suite" }
                $requiredCases = switch ($suite) {
                    'windows_n8' { @('n8_argv', 'n8_tools', 'n8_policy') }
                    'windows_n9' { @('n9_assets', 'n9_tools', 'n9_locale') }
                    'windows_n10' { @('n10_artifact', 'n10_install', 'n10_matrix') }
                    default { @() }
                }
                foreach ($case in $requiredCases) {
                    if ($listing -notmatch "(?m)^${case}: test$") { throw "Missing mandatory $suite case: $case" }
                }
                if ($suite -in @('windows_n8', 'windows_n9', 'windows_n10') -and [regex]::Matches($listing, '(?m)^.+: test$').Count -ne 3) { throw "Expected exactly three mandatory cases: $suite" }
                $ignored = Get-ToolText 'cargo' ((Get-CargoArguments 'test') + @('--package', 'fish', '--test', $suite, '--', '--ignored', '--list'))
                if ($ignored -match '(?m)^.+: test$') { throw "Ignored mandatory cases in $suite" }
                $result = Invoke-NativeSuite $suite ((Get-CargoArguments 'test') + @('--package', 'fish', '--test', $suite, '--', '--nocapture'))
                if ($suite -eq 'windows_n8') {
                    foreach ($pin in @(@('N8-ARGV', 11), @('N8-TOOLS', 20), @('N8-POLICY', 49))) {
                        $id, $families = $pin
                        $count = [int]$families * 2
                        if ($result -notmatch "DISCOVER $id families=$families profiles=2 ignored=0" -or $result -notmatch "EXECUTED $id discovered=$count executed=$count ignored=0 failed=0") { throw "Required native family/count pin failed: $id" }
                    }
                }
                if ($suite -eq 'windows_n9') {
                    $locale = if ($Features -eq 'default') { 'n9-locale-cases expected=19 executed=19 catalogs=localized help=embedded' } else { 'n9-locale-cases expected=21 executed=21 catalogs=English-only help=installed' }
                    if (-not $result.Contains($locale)) { throw 'Required N9 locale/profile case count did not execute' }
                    foreach ($pin in @(@('N9-ASSETS', 89), @('N9-TOOLS', 5))) {
                        $id, $families = $pin
                        $count = [int]$families * 2
                        if (-not $result.Contains("DISCOVER $id families=$families profiles=2 ignored=0") -or -not $result.Contains("EXECUTED $id discovered=$count executed=$count ignored=0 failed=0")) { throw "Required N9 family/count pin failed: $id" }
                    }
                }
                if ($suite -in @('windows_n8', 'windows_n9', 'windows_n10') -and -not $result.Contains('test result: ok. 3 passed; 0 failed; 0 ignored;')) { throw "Mandatory executed Rust case count mismatch: $suite" }
            }
            foreach ($crate in 'fish-build-helper', 'fish-color', 'fish-common', 'fish-fallback', 'fish-feature-flags', 'fish-fluent', 'fish-gettext-mo-file-parser', 'fish-printf', 'fish-tempfile', 'fish-util', 'fish-wcstringutil', 'fish-wgetopt', 'fish-widecharwidth', 'fish-widestring') {
                Invoke-Tool 'cargo' ((Get-CargoArguments 'test') + @('--package', $crate))
            }
            if ($Features -eq 'default') {
                foreach ($crate in 'fish-build-man-pages', 'fish-gettext', 'fish-gettext-maps', 'fish-localization') {
                    Invoke-Tool 'cargo' ((Get-CargoArguments 'test') + @('--package', $crate))
                }
            }
            $ftlArguments = (Get-CargoArguments 'test') + @('--package', 'xtask', '--test', 'fluent_rewrite')
            $ftlListing = Get-ToolText 'cargo' ($ftlArguments + @('--', '--list'))
            foreach ($case in 'unicode_rewrite_is_consumed_by_fluent_and_releases_temporary_files', 'denied_replacement_preserves_source_and_releases_temporary_files') {
                if ($ftlListing -notmatch "(?m)^${case}: test$") { throw "Missing native Fluent rewrite case: $case" }
            }
            if ([regex]::Matches($ftlListing, '(?m)^.+: test$').Count -ne 2) { throw 'Native Fluent rewrite discovery count mismatch' }
            $ftlIgnored = Get-ToolText 'cargo' ($ftlArguments + @('--', '--ignored', '--list'))
            if ($ftlIgnored -match '(?m)^.+: test$') { throw 'Ignored native Fluent rewrite case' }
            $ftlResult = Get-ToolText 'cargo' ($ftlArguments + @('--', '--nocapture', '--test-threads=1'))
            Write-Host $ftlResult
            if (-not $ftlResult.Contains('test result: ok. 2 passed; 0 failed; 0 ignored;')) { throw 'Native Fluent rewrite executed count mismatch' }
            Invoke-Tool 'cargo' ((Get-CargoArguments 'test') + @('--package', 'xtask', '--lib'))
            $gettextArguments = (Get-CargoArguments 'test') + @('--package', 'xtask', '--test', 'gettext_maintenance')
            $gettextListing = Get-ToolText 'cargo' ($gettextArguments + @('--', '--list'))
            if ($gettextListing -notmatch '(?m)^unicode_catalog_check_update_and_gnu_mo_consumer: test$' -or [regex]::Matches($gettextListing, '(?m)^.+: test$').Count -ne 1) { throw 'Native gettext maintenance discovery count mismatch' }
            $gettextIgnored = Get-ToolText 'cargo' ($gettextArguments + @('--', '--ignored', '--list'))
            if ($gettextIgnored -match '(?m)^.+: test$') { throw 'Ignored native gettext maintenance case' }
            $gettextResult = Get-ToolText 'cargo' ($gettextArguments + @('--', '--nocapture', '--test-threads=1'))
            Write-Host $gettextResult
            if (-not $gettextResult.Contains('test result: ok. 1 passed; 0 failed; 0 ignored;')) { throw 'Native gettext maintenance executed count mismatch' }
        }
        'Package' {
            if (-not $OutputDirectory) { throw 'Package requires -OutputDirectory' }
            if ($Configuration -ne 'release') { throw 'ZIP distribution is release-only; use Build/Check for debug artifacts (debug CRTs are not a redistributable channel)' }
            Build-Native
            $OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
            [IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null
            $stage = Join-Path $OutputDirectory ('.fish-stage-' + [guid]::NewGuid().ToString('N'))
            [IO.Directory]::CreateDirectory($stage) | Out-Null
            try {
                $bin = Join-Path $stage 'bin'
                [IO.Directory]::CreateDirectory($bin) | Out-Null
                $build = if ($env:CARGO_TARGET_DIR) { [IO.Path]::GetFullPath($env:CARGO_TARGET_DIR, $root) } else { Join-Path $root 'target' }
                $binaryDir = Join-Path $build "$target/$Configuration"
                $executables = [ordered]@{}
                foreach ($exe in 'fish', 'fish_indent', 'fish_key_reader') {
                    Copy-Item -LiteralPath (Join-Path $binaryDir "$exe.exe") -Destination $bin
                    $executables[$exe] = Inspect-Exe (Join-Path $bin "$exe.exe")
                }
                $versionText = Get-ToolText (Join-Path $bin 'fish.exe') @('--version')
                if ($versionText -notmatch '^fish, version (.+)$') { throw "Unexpected fish version: $versionText" }
                $version = $Matches[1]
                if ($version -notmatch '^[a-zA-Z0-9_.+-]+$') { throw 'Version is not a safe artifact filename' }
                foreach ($file in Get-Files (Join-Path $root 'share')) {
                    if ($file -match '(^|/)__pycache__/|\.py[co]$') { continue }
                    $destinationFile = Join-Path $stage "share/fish/$file"
                    [IO.Directory]::CreateDirectory((Split-Path $destinationFile -Parent)) | Out-Null
                    Copy-Item -LiteralPath (Join-Path $root "share/$file") -Destination $destinationFile
                }
                [IO.Directory]::CreateDirectory((Join-Path $stage 'etc/fish')) | Out-Null
                Copy-Item -LiteralPath (Join-Path $root 'etc/config.fish') -Destination (Join-Path $stage 'etc/fish/config.fish')
                $docs = Join-Path $stage 'share/doc/fish'
                [IO.Directory]::CreateDirectory($docs) | Out-Null
                foreach ($file in 'COPYING', 'README.rst', 'CHANGELOG.rst') { Copy-Item -LiteralPath (Join-Path $root $file) -Destination $docs }
                Copy-Item -LiteralPath (Join-Path $root 'doc_src/license.rst') -Destination $docs
                # Minimal runtime omits embedded help, not installed help. Sphinx is a
                # packaging-host dependency only; neither Python nor Sphinx is shipped.
                $help = Join-Path $stage 'share/doc/fish/help'
                Build-NativeHelp $help (Join-Path $stage '.doctrees-text') $version
                Remove-Item -LiteralPath (Join-Path $stage '.doctrees-text') -Recurse -Force
                Copy-Item -LiteralPath $PSCommandPath -Destination (Join-Path $stage 'native.ps1')
                $metadata = (Get-ToolText 'cargo' @('metadata', '--locked', '--format-version', '1')) | ConvertFrom-Json
                $notices = @()
                foreach ($package in @($metadata.packages | Sort-Object name, version)) {
                    $notice = [ordered]@{ name = $package.name; version = $package.version; license = $package.license; source = $package.source; repository = $package.repository; licenseFiles = @() }
                    $source = Split-Path $package.manifest_path -Parent
                    $licenseFiles = if ($package.source) { @(Get-ChildItem -LiteralPath $source -File -Recurse) } else { @(Get-ChildItem -LiteralPath $source -File) }
                    foreach ($license in @($licenseFiles | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([-._].*)?$' })) {
                        $relative = [IO.Path]::GetRelativePath($source, $license.FullName)
                        $dest = Join-Path $docs "licenses/$($package.name)-$($package.version)/$relative"
                        [IO.Directory]::CreateDirectory((Split-Path $dest -Parent)) | Out-Null
                        Copy-Item -LiteralPath $license.FullName -Destination $dest
                        $notice.licenseFiles += [IO.Path]::GetRelativePath($stage, $dest).Replace('\', '/')
                    }
                    $notices += $notice
                }
                Write-Json (Join-Path $docs 'third-party-notices.json') $notices
                $revision = 'unavailable (source archive or Git not installed)'
                if ((Test-Path -LiteralPath (Join-Path $root '.git')) -and (Get-Command git -ErrorAction SilentlyContinue)) {
                    $revision = Get-ToolText 'git' @('-C', $root, 'rev-parse', 'HEAD')
                }
                $sourceFiles = [Collections.Generic.List[string]]::new()
                foreach ($directory in 'src', 'crates', 'vendor/fluent-ftl-tools', 'share', 'etc', 'doc_src', 'build_tools', '.cargo', 'localization') {
                    foreach ($file in Get-Files (Join-Path $root $directory)) {
                        if ($file -notmatch '(^|/)__pycache__/|\.py[co]$') { $sourceFiles.Add("$directory/$file") }
                    }
                }
                foreach ($file in 'Cargo.toml', 'Cargo.lock', 'build.rs', 'COPYING', 'README.rst', 'CHANGELOG.rst', 'CONTRIBUTING.rst', 'pyproject.toml', 'uv.lock') { $sourceFiles.Add($file) }
                $sourceNames = $sourceFiles.ToArray()
                [Array]::Sort($sourceNames, [StringComparer]::Ordinal)
                $sourceInputs = @($sourceNames | ForEach-Object { [ordered]@{ path = $_; sha256 = (Get-FileHash -LiteralPath (Join-Path $root $_) -Algorithm SHA256).Hash.ToLowerInvariant() } })
                Write-Json (Join-Path $stage 'source-inputs.json') $sourceInputs
                $inputs = [ordered]@{
                    version = $version; target = $target; host = $target; configuration = $Configuration; features = $Features
                    revision = $revision; repository = 'https://github.com/fish-shell/fish-shell'
                    cflags = $env:CFLAGS; cc = $env:CC; pcre2LibDir = $env:PCRE2_SYS_LIB_DIR; sourceDateEpoch = $env:SOURCE_DATE_EPOCH
                    rust = (Get-ToolText 'rustc' @('-Vv')); cargo = (Get-ToolText 'cargo' @('-V')); linker = (Get-Item -LiteralPath (Get-Command link.exe).Source).VersionInfo.FileVersion
                    sdk = $env:WindowsSDKVersion; lockfileSha256 = (Get-FileHash -LiteralPath (Join-Path $root 'Cargo.lock')).Hash.ToLowerInvariant()
                    sourceInputsSha256 = (Get-FileHash -LiteralPath (Join-Path $stage 'source-inputs.json') -Algorithm SHA256).Hash.ToLowerInvariant()
                    rustflags = $env:RUSTFLAGS; pcre2 = 'static PCRE2-32 (PCRE2_SYS_STATIC=1)'; crt = 'static MSVC CRT; Windows system imports only'
                    executables = $executables; source = 'Version from actual fish --version; build-helper Git/archive policy'
                }
                Write-Json (Join-Path $stage 'build-inputs.json') $inputs
                Assert-PlainTree $stage
                $files = @(Get-Files $stage | ForEach-Object { [ordered]@{ path = $_; sha256 = (Get-FileHash -LiteralPath (Join-Path $stage $_) -Algorithm SHA256).Hash.ToLowerInvariant() } })
                Write-Json (Join-Path $stage 'package-manifest.json') ([ordered]@{ schema = 1; product = 'fish'; target = $target; version = $version; features = $Features; signing = 'unsigned'; files = $files })
                $null = Assert-Package $stage
                $final = Join-Path $OutputDirectory "fish-$version-windows-x64-$Features-unsigned.zip"
                if (Test-Path -LiteralPath $final) { throw "Refusing to overwrite artifact: $final" }
                $partial = "$final.partial"
                $partialStream = [IO.File]::Open($partial, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
                try {
                    try {
                        $writer = [IO.Compression.ZipArchive]::new($partialStream, [IO.Compression.ZipArchiveMode]::Create, $true)
                        try {
                            foreach ($file in Get-Files $stage) {
                                $entry = $writer.CreateEntry($file, [IO.Compression.CompressionLevel]::Optimal)
                                $entry.LastWriteTime = [DateTimeOffset]::new(1980, 1, 1, 0, 0, 0, [TimeSpan]::Zero)
                                $sourceStream = [IO.File]::OpenRead((Join-Path $stage $file))
                                try {
                                    $entryStream = $entry.Open()
                                    try { $sourceStream.CopyTo($entryStream) } finally { $entryStream.Dispose() }
                                } finally { $sourceStream.Dispose() }
                            }
                        } finally { $writer.Dispose() }
                    } finally { $partialStream.Dispose() }
                    [IO.File]::Move($partial, $final)
                } catch {
                    [IO.File]::Delete($partial)
                    throw
                }
                [IO.File]::WriteAllText("$final.sha256", ((Get-FileHash -LiteralPath $final).Hash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($final) + "`n"), $utf8)
                Write-Output $final
            } finally { if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force } }
        }
        { $_ -in 'Inspect', 'Install' } {
            if (-not $Archive) { throw "$Action requires -Archive" }
            $Archive = [IO.Path]::GetFullPath($Archive)
            if ($Action -eq 'Install' -and -not $Destination) { throw 'Install requires a new, explicit version directory via -Destination' }
            if ($Action -eq 'Install') {
                $Destination = [IO.Path]::GetFullPath($Destination)
                if (Test-Path -LiteralPath $Destination) { throw "Destination already exists; select a new version directory: $Destination" }
                $parent = Split-Path $Destination -Parent
                if (-not (Test-Path -LiteralPath $parent -PathType Container)) { throw "Create the installation parent first: $parent" }
            } else { $parent = [IO.Path]::GetTempPath() }
            $stage = Join-Path $parent ('.fish-extract-' + [guid]::NewGuid().ToString('N'))
            [IO.Directory]::CreateDirectory($stage) | Out-Null
            try {
                $manifest = Expand-Package $Archive $stage
                if ($Action -eq 'Install') { [IO.Directory]::Move($stage, $Destination); Write-Output $Destination }
                else { $manifest | ConvertTo-Json -Depth 30 }
            } finally { if (Test-Path -LiteralPath $stage) { Remove-Item -LiteralPath $stage -Recurse -Force } }
        }
        'Remove' {
            if (-not $Destination) { throw 'Remove requires -Destination' }
            $Destination = [IO.Path]::GetFullPath($Destination)
            $null = Assert-Package $Destination
            # Refuse unknown/modified files, reparse points and active locks before moving anything.
            $handles = @()
            try {
                foreach ($file in Get-Files $Destination) { $handles += [IO.File]::Open((Join-Path $Destination $file), [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None) }
            } finally { foreach ($handle in $handles) { $handle.Dispose() } }
            $tomb = $Destination + '.removing-' + [guid]::NewGuid().ToString('N')
            [IO.Directory]::Move($Destination, $tomb)
            try { Remove-Item -LiteralPath $tomb -Recurse -Force }
            catch { throw "Removal incomplete; remaining package files are at $tomb. User state was not touched. $($_.Exception.Message)" }
            Write-Output "Removed $Destination; independent user state was not changed."
        }
    }
} finally { Pop-Location }
