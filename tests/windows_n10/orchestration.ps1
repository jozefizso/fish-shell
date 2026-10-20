#requires -Version 7.4
param([Parameter(Mandatory)][string]$SourceRoot, [Parameter(Mandatory)][string]$OutsideDirectory)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
# Execute the production function itself without recursively launching Check's
# complete test suite from within that same suite. No replacement Cargo/tool is used.
$tokens = $null
$parseErrors = $null
$ast = [Management.Automation.Language.Parser]::ParseFile((Join-Path $SourceRoot 'build_tools/native.ps1'), [ref]$tokens, [ref]$parseErrors)
if ($parseErrors) { throw 'Native orchestration script did not parse' }
$definitions = @($ast.FindAll({ param($node) $node -is [Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Invoke-NativeSuite' }, $false))
if ($definitions.Count -ne 1) { throw 'Expected the one production native-suite function' }
. ([scriptblock]::Create($definitions[0].Extent.Text))
$root = [IO.Path]::GetFullPath($SourceRoot)
$Configuration = 'debug'
$Features = 'minimal'
$utf8 = [Text.UTF8Encoding]::new($false)
Set-Location -LiteralPath $OutsideDirectory
[Environment]::CurrentDirectory = $OutsideDirectory
$env:CARGO_TARGET_DIR = Join-Path $OutsideDirectory 'outside workspace results'
$text = Invoke-NativeSuite 'outside-workspace' @('metadata', '--locked', '--offline', '--no-deps', '--format-version', '1')
$json = @($text.Split("`n") | Where-Object { $_.StartsWith('{') })
if ($json.Count -ne 1) { throw 'Expected actual Cargo workspace metadata' }
$metadata = $json[0] | ConvertFrom-Json
if ([IO.Path]::GetFullPath($metadata.workspace_root) -ne $root) { throw "Captured Cargo selected the wrong workspace: $($metadata.workspace_root)" }
Write-Output 'N10 outside-workspace native orchestration passed'
