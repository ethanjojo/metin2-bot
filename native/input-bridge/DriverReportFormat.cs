using HIDMaestro;
using System.Text;
using System.Text.Json;

// Explicit streaming serialization is compatible with the NativeAOT bridge.
// Readiness is not evidence of absence. Only a successful fresh preview can
// describe current artifacts; removal reports contain history, not inventory.
internal static class DriverReportFormat
{
    internal static string Serialize(HMUninstallReport report)
    {
        var known = report.IsPreview && report.Status == HMUninstallStatus.Previewed
            && report.RunningPrograms.Count == 0
            && report.Entries.All(entry => entry.Outcome is HMUninstallOutcome.WouldRemove or HMUninstallOutcome.Kept);
        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream))
        {
            writer.WriteStartObject();
            writer.WriteNumber("schema_version", 1);
            writer.WriteBoolean("preview", report.IsPreview);
            writer.WriteString("status", report.Status.ToString());
            writer.WriteBoolean("presence_known", known);
            if (known)
                writer.WriteNumber("managed_artifact_count", report.Entries.Count(entry => entry.Kind != "service setting"));
            else
                writer.WriteNull("managed_artifact_count");
            writer.WriteNumber("running_program_count", report.RunningPrograms.Count);
            writer.WriteStartArray("entries");
            foreach (var entry in report.Entries)
            {
                writer.WriteStartObject();
                writer.WriteString("kind", entry.Kind);
                writer.WriteString("location", entry.Location);
                writer.WriteString("outcome", entry.Outcome.ToString());
                writer.WriteString("reason", entry.Reason);
                writer.WriteEndObject();
            }
            writer.WriteEndArray();
            writer.WriteEndObject();
        }
        return Encoding.UTF8.GetString(stream.ToArray());
    }
}
