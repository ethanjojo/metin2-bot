using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Xml.Linq;

if (args.Length != 3) throw new ArgumentException("Expected assembly, destination and inventory paths.");
var assembly = Assembly.LoadFrom(Path.GetFullPath(args[0]));
var destination = Path.GetFullPath(args[1]);
Directory.CreateDirectory(destination);
var inventory = new SortedDictionary<string, string>(StringComparer.Ordinal);
var items = new XElement("ItemGroup");
// Read resources only: never inspect SDK types, invoke methods or initialize controllers.
foreach (var name in assembly.GetManifestResourceNames())
{
    var target = Path.GetFullPath(Path.Combine(destination, name));
    if (!target.StartsWith(destination + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
        throw new InvalidDataException($"Unsafe embedded resource name: {name}");
    Directory.CreateDirectory(Path.GetDirectoryName(target)!);
    items.Add(new XElement("EmbeddedResource", new XAttribute("Include", target), new XElement("LogicalName", name), new XElement("WithCulture", "false")));
    using var source = assembly.GetManifestResourceStream(name)
        ?? throw new InvalidDataException($"Missing resource: {name}");
    using (var file = File.Create(target)) source.CopyTo(file);
    inventory.Add(name, Convert.ToHexStringLower(SHA256.HashData(File.ReadAllBytes(target))));
}
if (inventory.Count == 0) throw new InvalidDataException("SDK has no embedded resources.");
if (!Directory.GetFiles(destination, "*", SearchOption.AllDirectories).Select(path => Path.GetRelativePath(destination, path)).Order().SequenceEqual(inventory.Keys.Order()))
    throw new InvalidDataException("Unexpected stale resource files: use a clean verified cache.");
File.WriteAllText(args[2], JsonSerializer.Serialize(inventory, new JsonSerializerOptions { WriteIndented = true }));
new XDocument(new XElement("Project", items)).Save(Path.ChangeExtension(args[2], ".props"));
Console.WriteLine($"Extracted {inventory.Count} pinned SDK resources.");
