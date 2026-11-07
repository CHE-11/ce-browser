<#
.SYNOPSIS
Calls a passage tool on the running ce-browser dev build (`pnpm tauri dev`).

.EXAMPLE
./scripts/passage.ps1 health
./scripts/passage.ps1 logs '{"stream":"console","limit":50}'
./scripts/passage.ps1 eval '{"label":"ui","expression":"document.title"}'
./scripts/passage.ps1 screenshot
#>
param(
  [Parameter(Mandatory = $true, Position = 0)][string]$Tool,
  [Parameter(Position = 1)][string]$Json = "{}"
)

$port = if ($env:CE_PASSAGE_PORT) { $env:CE_PASSAGE_PORT } else { 1427 }
$base = "http://127.0.0.1:$port"

try {
  if ($Tool -eq "health") {
    $response = Invoke-RestMethod -Uri "$base/health" -Method Get
  } else {
    $body = [System.Text.Encoding]::UTF8.GetBytes($Json)
    $response = Invoke-RestMethod -Uri "$base/call/$Tool" -Method Post -ContentType "application/json; charset=utf-8" -Body $body
  }
} catch {
  if ($_.ErrorDetails -and $_.ErrorDetails.Message) {
    Write-Output $_.ErrorDetails.Message
  } else {
    Write-Output "passage is not reachable on $base. Start the app with 'pnpm tauri dev'. ($($_.Exception.Message))"
  }
  exit 1
}

$response | ConvertTo-Json -Depth 30
