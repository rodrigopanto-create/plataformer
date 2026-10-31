$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Push-Location $root
try {
    cargo build --locked --release --target wasm32-unknown-unknown
    if ($LASTEXITCODE -ne 0) { throw "The WASM build failed." }

    $metadataJson = cargo metadata --locked --format-version 1
    if ($LASTEXITCODE -ne 0) { throw "Unable to locate the Macroquad loader." }
    $metadata = ($metadataJson -join "`n") | ConvertFrom-Json
    $macroquad = @($metadata.packages | Where-Object { $_.name -eq "macroquad" })
    if ($macroquad.Count -ne 1) { throw "Expected exactly one Macroquad dependency." }
    $packageDir = Split-Path $macroquad[0].manifest_path -Parent
    $dist = Join-Path $root "dist"
    New-Item -ItemType Directory -Path $dist -Force | Out-Null
    Copy-Item (Join-Path $root "web\*") $dist -Recurse -Force
    Copy-Item (Join-Path $metadata.target_directory "wasm32-unknown-unknown\release\platformer.wasm") $dist
    $loader = Get-Content (Join-Path $packageDir "js\mq_js_bundle.js") -Raw
    # Macroquad 0.4.16's bundled quad_net callback lacks a declaration in strict mode.
    $loader = $loader.Replace(
        "register_plugin=function(e){e.env.ws_connect=",
        "var register_plugin;register_plugin=function(e){e.env.ws_connect="
    )
    [System.IO.File]::WriteAllText((Join-Path $dist "mq_js_bundle.js"), $loader,
        [System.Text.UTF8Encoding]::new($false))
    Copy-Item (Join-Path $packageDir "LICENSE-MIT") (Join-Path $dist "macroquad-LICENSE-MIT.txt")

    $hashes = Get-ChildItem $dist -File -Recurse |
        Sort-Object FullName |
        Get-FileHash -Algorithm SHA256
    $sha256 = [System.Security.Cryptography.SHA256]::Create()
    try {
        $hash = $sha256.ComputeHash([System.Text.Encoding]::UTF8.GetBytes(($hashes.Hash -join "")))
        $buildId = ([System.BitConverter]::ToString($hash)).Replace("-", "").ToLowerInvariant()
    } finally {
        $sha256.Dispose()
    }
    $workerPath = Join-Path $dist "sw.js"
    $worker = (Get-Content $workerPath -Raw).Replace("__BUILD_ID__", $buildId)
    [System.IO.File]::WriteAllText($workerPath, $worker, [System.Text.UTF8Encoding]::new($false))
    Write-Host "Web build ready in $dist"
} finally {
    Pop-Location
}
