param([string]$Root=([IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))))
$ErrorActionPreference='Stop'
$source=[IO.File]::ReadAllText((Join-Path $Root 'src/updater.ps1'))
$start=$source.IndexOf('function Get-QueryFailure(')
$end=$source.IndexOf("if (`$Mode -eq 'Query') {",$start)
Invoke-Expression $source.Substring($start,$end-$start)
$passed=0
function Check($Condition,[string]$Name) {if (!$Condition) {throw $Name};$script:passed++;Write-Output "OK: $Name"}
$r=Get-QueryFailure 404 @{} 1000
Check ($r.status -eq 'no-release' -and $r.retry -eq 0) '404 is a normal missing release'
$r=Get-QueryFailure 429 @{'Retry-After'='3600'} 1000
Check ($r.status -eq 'limited' -and $r.retry -eq 4600) '429 Retry-After seconds'
$r=Get-QueryFailure 403 @{'X-RateLimit-Remaining'='0';'X-RateLimit-Reset'='6000'} 1000
Check ($r.status -eq 'limited' -and $r.retry -eq 6000) '403 respects server reset'
$r=Get-QueryFailure 429 @{'Retry-After'='Thu, 01 Jan 1970 02:00:00 GMT'} 1000
Check ($r.retry -eq 7200) 'Retry-After HTTP date'
$r=Get-QueryFailure 429 @{'Retry-After'='invalid';'X-RateLimit-Reset'='invalid'} 1000
Check ($r.retry -eq 1900) 'Malformed headers use bounded backoff'
$r=Get-QueryFailure 500 @{} 1000
Check ($r.status -eq 'http-error' -and $r.retry -eq 0) 'Server errors remain distinct'
$r=Get-QueryFailure 429 @{'Retry-After'='9223372036854775807'} 1000
Check ($r.retry -eq 605800) 'Untrusted headers cannot overflow'
$tokens=$null;$errors=$null
$null=[Management.Automation.Language.Parser]::ParseFile((Join-Path $Root 'src/updater.ps1'),[ref]$tokens,[ref]$errors)
Check ($errors.Count -eq 0) 'PowerShell helper parses correctly'
Write-Output "$passed query checks passed"
