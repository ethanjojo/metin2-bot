function ConvertTo-ExplicitDriverSource {
    [CmdletBinding()]
    param([Parameter(Mandatory)][string]$Source, [Parameter(Mandatory)][string]$AdapterDirectory)
    $original = [IO.File]::ReadAllText((Join-Path $AdapterDirectory 'EnsureForCreate.original.txt')).Replace("`r`n", "`n")
    $replacement = [IO.File]::ReadAllText((Join-Path $AdapterDirectory 'EnsureForCreate.replacement.txt')).Replace("`r`n", "`n")
    $normalized = $Source.Replace("`r`n", "`n")
    if (($normalized.Split([string[]]@($original), [StringSplitOptions]::None)).Count -ne 2) {
        throw 'Pinned HIDMaestro EnsureForCreate source changed; review the explicit-install adapter.'
    }
    return $normalized.Replace($original, $replacement)
}
Export-ModuleMember -Function ConvertTo-ExplicitDriverSource
