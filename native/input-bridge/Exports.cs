using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using HIDMaestro;
using HIDMaestro.Internal;

// Only this ABI crosses into Rust. Exceptions never cross a native boundary.
internal static unsafe class Exports
{
    private static readonly object Gate = new();
    private static readonly Dictionary<nuint, Session> Sessions = new();
    private static nuint nextHandle;
    private static void RequireInstalled()
    {
        if (RuntimeInformation.OSArchitecture != Architecture.X64) throw new PlatformNotSupportedException("Companion requires native x64 Windows.");
        // Canonical pinned SDK check: actual driver-store presence plus its own
        // payload fingerprint. The source adapter exposes this read-only gate.
        if (!DriverBuilder.WillTakeFastPath())
            throw new InvalidOperationException("HIDMaestro 2.1.0 input driver is missing or needs repair. Run the Companion Windows installer.");
    }
    private static int Call(byte* output, uint capacity, Func<string> action)
    {
        // Always leave a usable message buffer, including malformed ABI calls.
        if (output == null || capacity < 2 || capacity > 1024 * 1024) return 1;
        string message;
        int result;
        try { lock (Gate) message = action(); result = 0; }
        catch (Exception ex) { message = ex.Message; result = 1; }
        var target = new Span<byte>(output, checked((int)capacity));
        var bytes = Encoding.UTF8.GetBytes(message);
        int written = Math.Min(bytes.Length, target.Length - 1);
        while (written > 0 && written < bytes.Length && (bytes[written] & 0xC0) == 0x80) written--;
        bytes.AsSpan(0, written).CopyTo(target);
        target[written] = 0;
        return result;
    }
    [UnmanagedCallersOnly(EntryPoint = "mi_abi_version", CallConvs = [typeof(CallConvCdecl)])]
    public static uint AbiVersion() => 1;

    [UnmanagedCallersOnly(EntryPoint = "mi_manage", CallConvs = [typeof(CallConvCdecl)])]
    public static int Manage(uint operation, byte* output, uint capacity) => Call(output, capacity, () =>
    {
        switch (operation)
        {
            case 0: RequireInstalled(); return "HIDMaestro 2.1.0 input driver is installed.";
            case 1:
                using (var context = new HMContext())
                {
                    context.InstallDriver(); RequireInstalled();
                    // Verify that Windows can open the actual devices before
                    // setup reports success. Neutral reports only, no input.
                    using (var verified = new Session()) { }
                    return $"HIDMaestro 2.1.0: {context.LastDriverInstall}; virtual devices verified";
                }
            case 2:
            case 3:
                if (Sessions.Count != 0) throw new InvalidOperationException("Close active input sessions before driver removal.");
                var report = operation == 2 ? HMUninstall.Preview() : HMUninstall.Run();
                var detail = DriverReportFormat.Serialize(report);
                if (report.Status is HMUninstallStatus.Blocked or HMUninstallStatus.Failed) throw new InvalidOperationException(detail);
                return detail;
            default: throw new ArgumentException("Unknown input-driver operation");
        }
    });

    [UnmanagedCallersOnly(EntryPoint = "mi_open", CallConvs = [typeof(CallConvCdecl)])]
    public static int Open(nuint* handle, byte* output, uint capacity) => Call(output, capacity, () =>
    {
        if (handle == null) throw new ArgumentException("Missing session handle");
        *handle = 0;
        RequireInstalled();
        var session = new Session();
        var id = checked(++nextHandle);
        Sessions.Add(id, session);
        *handle = id;
        return "Virtual HID keyboard and mouse ready";
    });
    [UnmanagedCallersOnly(EntryPoint = "mi_keyboard", CallConvs = [typeof(CallConvCdecl)])]
    public static int Keyboard(nuint handle, byte modifiers, byte* keys, uint count, byte* output, uint capacity) => Call(output, capacity, () =>
    {
        if (count > 6 || (count != 0 && keys == null)) throw new ArgumentException("Invalid keyboard report");
        var values = new HMKey[count];
        for (int i = 0; i < count; i++)
        {
            if (keys[i] < 4 || keys[i] >= 0xE0) throw new ArgumentException("Invalid HID key usage");
            values[i] = (HMKey)keys[i];
        }
        Get(handle).Keyboard.SubmitKeyboard((HMKeyModifiers)modifiers, values);
        return "";
    });
    [UnmanagedCallersOnly(EntryPoint = "mi_mouse", CallConvs = [typeof(CallConvCdecl)])]
    public static int Mouse(nuint handle, int x, int y, uint buttons, byte* output, uint capacity) => Call(output, capacity, () =>
    {
        if (buttons > 1 || Math.Abs((long)x) > 127 || Math.Abs((long)y) > 127) throw new ArgumentException("Invalid mouse report");
        Get(handle).Mouse.SubmitMouse(new HMMouseState { X = x, Y = y, Buttons = (HMMouseButtons)buttons });
        return "";
    });
    [UnmanagedCallersOnly(EntryPoint = "mi_close", CallConvs = [typeof(CallConvCdecl)])]
    public static int Close(nuint handle, byte* output, uint capacity) => Call(output, capacity, () =>
    {
        var session = Get(handle);
        // Retain the handle on failed disposal so a caller can retry cleanup.
        session.Dispose(); Sessions.Remove(handle); return "Virtual input released";
    });
    private static Session Get(nuint handle) => Sessions.TryGetValue(handle, out var session) ? session : throw new ArgumentException("Invalid input session");
    private sealed class Session : IDisposable
    {
        private readonly HMContext context = new();
        public HMController Keyboard { get; private set; } = null!;
        public HMController Mouse { get; private set; } = null!;
        public Session()
        {
            try
            {
                Keyboard = context.CreateController(Profile(true));
                Mouse = context.CreateController(Profile(false));
                if (!Keyboard.DeliversEveryReport || !Keyboard.ReleasesOnConsumerExit || !Mouse.DeliversEveryReport || !Mouse.ReleasesOnConsumerExit)
                    throw new InvalidOperationException("Driver does not support reliable input release");
                var deadline = Environment.TickCount64 + 10000;
                while (Keyboard.KeyboardLeds == null && Environment.TickCount64 < deadline) Thread.Sleep(20);
                if (Keyboard.KeyboardLeds == null) throw new TimeoutException("Windows did not open the virtual keyboard");
                Thread.Sleep(300);
            }
            catch { context.Dispose(); throw; }
        }
        private static HMProfile Profile(bool keyboard)
        {
            var type = keyboard ? "keyboard" : "mouse";
            var builder = new HidDescriptorBuilder();
            if (keyboard) builder.Keyboard(); else builder.Mouse();
            return new HMProfileBuilder().Id("metinbot-" + type).Name("Companion virtual " + type)
                .Vendor("Companion").Vid(0xBEEF).Pid(keyboard ? (ushort)0x6761 : (ushort)0x6762)
                .ProductString("Companion virtual " + type).ManufacturerString("Companion")
                .Type(type).Connection("usb").FromDescriptorBuilder(builder).Build();
        }
        public void Dispose()
        {
            // Context owns both devices and attempts each teardown even if one fails.
            context.Dispose();
        }
    }
}
