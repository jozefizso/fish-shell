#requires -Version 7.4
param([Parameter(Mandatory)][string]$InstallRoot, [Parameter(Mandatory)][string]$StateRoot)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$start = [Diagnostics.ProcessStartInfo]::new((Join-Path $InstallRoot 'bin/fish.exe'))
$start.UseShellExecute = $false
$start.RedirectStandardInput = $true
$start.RedirectStandardOutput = $true
$start.RedirectStandardError = $true
$start.WorkingDirectory = $StateRoot
foreach ($arg in '-N', '-c', 'echo __N10_READY; read -l value') { $start.ArgumentList.Add($arg) }
$system = [IO.Path]::GetFullPath($env:SystemRoot)
$start.Environment.Clear()
$start.Environment['SystemRoot'] = $system
$start.Environment['PATH'] = Join-Path $system 'System32'
$start.Environment['HOME'] = Join-Path $StateRoot 'home'
$start.Environment['XDG_CONFIG_HOME'] = Join-Path $StateRoot 'config'
$start.Environment['XDG_DATA_HOME'] = Join-Path $StateRoot 'data'
$start.Environment['XDG_CACHE_HOME'] = Join-Path $StateRoot 'cache'
$process = [Diagnostics.Process]::new()
$process.StartInfo = $start
$started = $false
try {
    if (-not $process.Start()) { throw 'Native fish did not start' }
    $started = $true
    $errors = $process.StandardError.ReadToEndAsync()
    $ready = $process.StandardOutput.ReadLineAsync()
    if (-not $ready.Wait(5000) -or $ready.Result -ne '__N10_READY') { throw 'Native dependency fixture did not reach the evaluator handshake' }
    $output = $process.StandardOutput.ReadToEndAsync()
    $process.Refresh()
    $paths = @($process.Modules | ForEach-Object { $_.FileName })
    if ($paths.Count -lt 2) { throw 'Expected executable and Windows dependency modules' }
    $image = [IO.Path]::GetFullPath((Join-Path $InstallRoot 'bin/fish.exe'))
    foreach ($path in $paths) {
        if (-not $path.Equals($image, [StringComparison]::OrdinalIgnoreCase) -and -not $path.StartsWith($system.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Undeployed/non-system runtime dependency loaded: $path"
        }
    }
    $process.StandardInput.WriteLine('done')
    $process.StandardInput.Close()
    if (-not $process.WaitForExit(5000)) { throw 'Native dependency fixture did not exit' }
    if (-not $errors.Wait(5000) -or -not $output.Wait(5000)) { throw 'Native dependency fixture streams did not close' }
    if ($process.ExitCode -ne 0 -or $errors.Result) { throw "Native dependency fixture failed: $($errors.Result)" }
    $paths | ConvertTo-Json
} finally {
    if ($started -and -not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
    $process.Dispose()
}
