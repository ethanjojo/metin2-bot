using HIDMaestro.Internal;
using System.Security.Cryptography;
using System.Text.Json;

// This assembly uses the SDK's existing upstream test friendship. No contexts,
// native devices, deployment tools, certificate stores or services are opened.
var machine = new FakeMachine();
DriverReportChecks.Run();
DriverDeploy.Machine = machine;
foreach (var state in new[] { "missing", "old", "current", "newer", "old", "missing", "current" })
{
    machine.State = state;
    var refused = false;
    try
    {
        if (DriverDeploy.EnsureForCreate("offline-policy-fixture")) throw new Exception("Create reported deployment.");
    }
    catch (InvalidOperationException error) when (error.Message.Contains("Companion installer")) { refused = true; }
    if (refused != (state is "missing" or "old")) throw new Exception($"Wrong gate result: {state}");
    if (machine.Held || machine.Writes != 0) throw new Exception("Gate leaked its lock or mutated the machine.");
}
if (machine.LockCount != 7 || machine.StoreChecks != 7) throw new Exception("Gate skipped the fresh under-lock check.");
var inventory = JsonSerializer.Deserialize<SortedDictionary<string, string>>(File.ReadAllText(args[0]))!;
var assembly = typeof(DriverDeploy).Assembly;
if (!assembly.GetManifestResourceNames().Order().SequenceEqual(inventory.Keys.Order())) throw new Exception("Resource name inventory changed.");
foreach (var (name, expected) in inventory)
{
    using var resource = assembly.GetManifestResourceStream(name)!;
    if (Convert.ToHexStringLower(SHA256.HashData(resource)) != expected) throw new Exception($"Resource bytes changed: {name}");
}
Console.WriteLine($"SDK adapter PASS: seven missing/old/current/newer gate cases under lock, zero mutations, {inventory.Count} identical resources.");

sealed class FakeMachine : DriverDeploy.DeployMachine
{
    public string State = "missing";
    public bool Held;
    public int Writes, LockCount, StoreChecks;
    internal override IDisposable Lock()
    {
        if (Held) throw new Exception("Nested lock.");
        Held = true; LockCount++;
        return new Releaser(this);
    }
    void CheckLock() { if (!Held) throw new Exception("State read outside deploy lock."); }
    internal override bool StoreHasBothPackages() { CheckLock(); StoreChecks++; return State != "missing"; }
    internal override List<DriverDeploy.DriverStamp> Stamps(string infName)
    {
        CheckLock();
        return [new(new DateTime(2026, 1, 1), State switch { "old" => new Version(2, 0, 0, 0), "newer" => new Version(3, 0, 0, 0), _ => new Version(2, 1, 0, 0) })];
    }
    internal override DriverDeploy.DriverStamp Embedded(string infName) { CheckLock(); return new(new DateTime(2026, 1, 1), new Version(2, 1, 0, 0)); }
    void ForbidMutation() { Writes++; throw new Exception("Create attempted a machine mutation."); }
    internal override void Deploy() => ForbidMutation();
    internal override void AddPackages(IReadOnlyCollection<string> infs) => ForbidMutation();
    internal override void RecordPayload() => ForbidMutation();
    internal override void PruneStaging() => ForbidMutation();
    internal override void MarkConfirmed() => ForbidMutation();
    internal override string? ManifestHash() => throw new Exception("Create trusted cached state.");
    internal override bool FastPath() => throw new Exception("Create skipped fresh checks.");
    internal override OwnerCensus Census() => throw new Exception("Create entered deployment policy.");
    sealed class Releaser(FakeMachine machine) : IDisposable { public void Dispose() => machine.Held = false; }
}
