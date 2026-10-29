param(
    [Parameter(Mandatory=$true)][ValidateSet('Query','Apply')][string]$Mode,
    [string]$Endpoint,
    [string]$AppVersion,
    [string]$Manifest
)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

function Open-Https([string]$Url, [string]$Agent) {
    for ($redirect = 0; $redirect -le 5; $redirect++) {
        $uri = [Uri]$Url
        $allowed = @('api.github.com','github.com','release-assets.githubusercontent.com',
            'objects.githubusercontent.com','github-releases.githubusercontent.com')
        if ($uri.Scheme -ne 'https' -or $uri.Port -ne 443 -or $uri.UserInfo -or
            $uri.Host -notin $allowed) { throw 'Destino HTTPS no permitido' }
        $request = [Net.HttpWebRequest]::Create($uri)
        $request.UserAgent = $Agent
        $request.Accept = 'application/octet-stream'
        if ($uri.Host -eq 'api.github.com') {
            $request.Accept = 'application/vnd.github+json'
            $request.Headers.Add('X-GitHub-Api-Version','2022-11-28')
        }
        $request.AllowAutoRedirect = $false
        $request.Timeout = 10000
        $request.ReadWriteTimeout = 15000
        if ($Mode -eq 'Apply') {
            # HttpWebRequest.Timeout does not apply to the asynchronous API.
            # Bound the connect/TLS wait ourselves and keep Cancel responsive.
            $pending = $request.BeginGetResponse($null,$null)
            $connect = [Diagnostics.Stopwatch]::StartNew()
            try {
                while (!$pending.IsCompleted) {
                    [Windows.Forms.Application]::DoEvents()
                    if ($form.IsDisposed) { $request.Abort(); throw [OperationCanceledException]::new() }
                    if ($connect.Elapsed.TotalSeconds -gt 10) { $request.Abort(); throw 'Tiempo de conexión agotado' }
                    Start-Sleep -Milliseconds 30
                }
                $response = $request.EndGetResponse($pending)
            } finally { $pending.AsyncWaitHandle.Close() }
        } else { $response = $request.GetResponse() }
        if ([int]$response.StatusCode -in @(301,302,303,307,308)) {
            $location = $response.Headers['Location']
            $response.Dispose()
            if (!$location) { throw 'Redirección sin destino' }
            $Url = ([Uri]::new($uri, $location)).AbsoluteUri
            continue
        }
        if ([int]$response.StatusCode -ne 200) { $response.Dispose(); throw 'Respuesta HTTP inesperada' }
        return $response
    }
    throw 'Demasiadas redirecciones'
}

function Get-QueryFailure([int]$Code, $Headers, [long]$Now) {
                $code=[int]$Code
                $status='http-error'
                $retry=[long]0
                if ($code -eq 404) {$status='no-release'}
                elseif ($code -eq 429 -or ($code -eq 403 -and
                    ($Headers['X-RateLimit-Remaining'] -eq '0' -or $Headers['Retry-After']))) {
                    $status='limited'
                    $retry=$now+900
                    $seconds=[long]0
                    if ([long]::TryParse($Headers['Retry-After'],[ref]$seconds) -and $seconds -gt 0) {$retry=[Math]::Max($retry,$now+[Math]::Min($seconds,604800))}
                    else {
                        $date=[DateTimeOffset]::MinValue
                        if ([DateTimeOffset]::TryParse($Headers['Retry-After'],[ref]$date)) {$retry=[Math]::Max($retry,[Math]::Min($date.ToUnixTimeSeconds(),$now+604800))}
                    }
                    $reset=[long]0
                    if ([long]::TryParse($Headers['X-RateLimit-Reset'],[ref]$reset)) {$retry=[Math]::Max($retry,[Math]::Min($reset,$now+604800))}
                }
                return @{status=$status;http=$code;retry=$retry}
}

if ($Mode -eq 'Query') {
    $response = $null
    $stream = $null
    try {
        $response = Open-Https $Endpoint "Enigmax-CinePlayer-Updater/$AppVersion"
        $stream = $response.GetResponseStream()
        $memory = New-Object IO.MemoryStream
        $queryTime = [Diagnostics.Stopwatch]::StartNew()
        $buffer = New-Object byte[] 32768
        while (($count = $stream.Read($buffer,0,$buffer.Length)) -gt 0) {
            if ($queryTime.Elapsed.TotalSeconds -gt 30) { throw 'Tiempo de consulta agotado' }
            if ($memory.Length + $count -gt 2097152) { throw 'JSON demasiado grande' }
            $memory.Write($buffer,0,$count)
        }
        $release = [Text.Encoding]::UTF8.GetString($memory.ToArray()) | ConvertFrom-Json
        @{status='ok';release=$release} |
            ConvertTo-Json -Depth 32 -Compress
        exit 0
    } catch {
        $failure=$_.Exception
        while ($failure.InnerException -and !($failure -is [Net.WebException])) {$failure=$failure.InnerException}
        $http=$null
        if ($failure -is [Net.WebException]) {$http=$failure.Response}
        if ($http) {
            try {
                Get-QueryFailure ([int]$http.StatusCode) $http.Headers ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds()) | ConvertTo-Json -Compress
                exit 0
            } finally {$http.Dispose()}
        }
        [Console]::Error.WriteLine($_.Exception.Message); exit 1
    }
    finally { if ($stream) {$stream.Dispose()}; if ($response) {$response.Dispose()}; if ($memory) {$memory.Dispose()} }
}

