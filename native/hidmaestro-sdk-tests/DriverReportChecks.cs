using HIDMaestro;
using System.Text.Json;

internal static class DriverReportChecks
{
    internal static void Run()
    {
        Check(new(true, HMUninstallStatus.Previewed, [], []), true, 0);
        Check(new(true, HMUninstallStatus.Previewed, [], [new("driver package", "older.inf", HMUninstallOutcome.WouldRemove, null)]), true, 1);
        Check(new(true, HMUninstallStatus.Previewed, [], [new("certificate", "Root/test", HMUninstallOutcome.Kept, "shared package")]), true, 1);
        Check(new(true, HMUninstallStatus.Previewed, [], [new("service setting", "GameInputSvc", HMUninstallOutcome.Kept, "previous startup not recorded")]), true, 0);
        Check(new(true, HMUninstallStatus.Failed, [], [new("removal step", "scan", HMUninstallOutcome.Failed, "denied")]), false, null);
        Check(new(true, HMUninstallStatus.Blocked, [new(123, "other.exe", "2.1.0")], []), false, null);
        Check(new(true, HMUninstallStatus.Previewed, [], [new("folder", "unknown", HMUninstallOutcome.Failed, "denied")]), false, null);
        Check(new(false, HMUninstallStatus.Removed, [], [new("driver package", "older.inf", HMUninstallOutcome.Removed, null)]), false, null);
        Console.WriteLine("Driver report PASS: empty, old/shared artifacts, retained service setting, unknown/failed/blocked scans and removal history.");
    }

    private static void Check(HMUninstallReport report, bool known, int? count)
    {
        using var json = JsonDocument.Parse(DriverReportFormat.Serialize(report));
        var root = json.RootElement;
        if (root.GetProperty("schema_version").GetInt32() != 1 || root.GetProperty("presence_known").GetBoolean() != known)
            throw new Exception("Driver report misrepresented inventory certainty.");
        var actual = root.GetProperty("managed_artifact_count");
        if (count is null ? actual.ValueKind != JsonValueKind.Null : actual.GetInt32() != count)
            throw new Exception("Driver report confused readiness/removal history with artifact absence.");
        if (root.GetProperty("entries").GetArrayLength() != report.Entries.Count)
            throw new Exception("Driver report discarded artifact detail.");
    }
}
