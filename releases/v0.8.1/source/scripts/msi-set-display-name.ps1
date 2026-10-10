<#
.SYNOPSIS
  把 MSI 在「设置 -> 应用」卸载列表里的显示名改为「产品名 v版本号」。

.DESCRIPTION
  Tauri 的 WiX 打包没有对应配置项，MSI 的显示名取自 Property 表的 ProductName。
  这里在打包完成后用 Windows Installer COM 接口改写它（不改 productName 本身：
  它还决定升级判断、安装目录与快捷方式名）。仅修改传入的文件；MSI 未做代码签名，
  修改后无需重新签名。

.EXAMPLE
  powershell -NoProfile -File scripts\msi-set-display-name.ps1 -Path .\FindFlow.msi -Name "FindFlow v0.8.0"
#>
param(
  [Parameter(Mandatory = $true)][string]$Path,
  [Parameter(Mandatory = $true)][string]$Name
)

$ErrorActionPreference = 'Stop'
$full = (Resolve-Path -LiteralPath $Path).Path
$installer = New-Object -ComObject WindowsInstaller.Installer

function Get-Prop($db, $prop) {
  $view = $db.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $db, @("SELECT `Value` FROM `Property` WHERE `Property`='$prop'"))
  $view.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $view, $null) | Out-Null
  $rec = $view.GetType().InvokeMember('Fetch', 'InvokeMethod', $null, $view, $null)
  $val = $null
  if ($rec) { $val = $rec.GetType().InvokeMember('StringData', 'GetProperty', $null, $rec, 1) }
  $view.GetType().InvokeMember('Close', 'InvokeMethod', $null, $view, $null) | Out-Null
  return $val
}

# 1 = msiOpenDatabaseModeTransact（可写，Commit 后落盘）
$db = $installer.GetType().InvokeMember('OpenDatabase', 'InvokeMethod', $null, $installer, @($full, 1))
$before = Get-Prop $db 'ProductName'
$safe = $Name.Replace("'", "''")
$view = $db.GetType().InvokeMember('OpenView', 'InvokeMethod', $null, $db, @("UPDATE `Property` SET `Value`='$safe' WHERE `Property`='ProductName'"))
$view.GetType().InvokeMember('Execute', 'InvokeMethod', $null, $view, $null) | Out-Null
$view.GetType().InvokeMember('Close', 'InvokeMethod', $null, $view, $null) | Out-Null
$db.GetType().InvokeMember('Commit', 'InvokeMethod', $null, $db, $null) | Out-Null
$after = Get-Prop $db 'ProductName'
[System.Runtime.InteropServices.Marshal]::ReleaseComObject($db) | Out-Null

if ($after -ne $Name) { throw "写入失败：ProductName 现为 '$after'，期望 '$Name'" }
Write-Output "ProductName: '$before' -> '$after'"
