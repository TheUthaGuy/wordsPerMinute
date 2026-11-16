# Writes a week of fake typing sessions so the dashboard has something to show.
# Only used by the screenshot workflow; never touches a real user's data.
$ErrorActionPreference = 'Stop'

$dataDir = Join-Path $env:APPDATA 'WpmCounter\data'
New-Item -ItemType Directory -Force -Path $dataDir | Out-Null

$rand = New-Object System.Random 42
$names = @('Email replies', 'Design doc', 'Code review notes', '', 'Blog draft', 'Chat', '', 'Bug report')
$sessions = @()
$now = Get-Date

for ($day = 6; $day -ge 0; $day--) {
    $count = 2 + $rand.Next(3)
    for ($i = 0; $i -lt $count; $i++) {
        $start = $now.Date.AddDays(-$day).AddHours(9 + $i * 2.5).AddMinutes($rand.Next(50))
        if ($start -gt $now.AddMinutes(-30)) { $start = $now.AddMinutes(-30 - 40 * $i) }
        $active = 240 + $rand.Next(900)
        $length = [int]($active * (1.2 + $rand.NextDouble() * 0.6))
        $baseWpm = 55 + $rand.Next(35)
        $typed = [int]($baseWpm * 5 * $active / 60)
        $separators = [int]($typed / 6)
        $chars = $typed - $separators
        $backspaces = [int]($typed * (0.03 + $rand.NextDouble() * 0.05))

        $timeline = @()
        $peak = 0
        for ($t = 0; $t -le $length; $t += 15) {
            $w = [math]::Max(0, $baseWpm + 18 * [math]::Sin($t / 70.0) + $rand.Next(-10, 11))
            $peak = [math]::Max($peak, $w)
            $timeline += , @([double]$t, [double][math]::Round($w, 1))
        }

        $sessions += [ordered]@{
            id          = [long]([DateTimeOffset]$start).ToUnixTimeMilliseconds()
            name        = $names[$rand.Next($names.Count)]
            start       = $start.ToString('yyyy-MM-ddTHH:mm:ss.fffzzz')
            end         = $start.AddSeconds($length).ToString('yyyy-MM-ddTHH:mm:ss.fffzzz')
            active_secs = [double]$active
            chars       = $chars
            separators  = $separators
            words       = $separators
            keystrokes  = $typed + $backspaces
            backspaces  = $backspaces
            peak_wpm    = [double]$peak
            timeline    = $timeline
        }
    }
}

$sessions = $sessions | Sort-Object { $_.start }
ConvertTo-Json -InputObject @($sessions) -Depth 6 |
    Set-Content -Encoding utf8NoBOM -Path (Join-Path $dataDir 'sessions.json')
Write-Host "Seeded $($sessions.Count) demo sessions into $dataDir"
