// Prints the decompiled body of the function containing the given address.
// Usage (with the persistent project, no re-import):
//   analyzeHeadless D:\temp\ghidra-projects mngbase -process mngbase.dll `
//     -noanalysis -scriptPath tools\rea\ghidra -postScript DecompileAt.java 0x1802ae0c6
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;

public class DecompileAt extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            println("usage: DecompileAt.java <address>");
            return;
        }
        Address address = currentProgram
                .getAddressFactory()
                .getDefaultAddressSpace()
                .getAddress(args[0]);
        Function function = getFunctionContaining(address);
        if (function == null) {
            println("no function at " + address);
            return;
        }
        DecompInterface decompiler = new DecompInterface();
        decompiler.openProgram(currentProgram);
        DecompileResults results = decompiler.decompileFunction(function, 120, monitor);
        if (results == null || !results.decompileCompleted()) {
            println("decompile failed: " + (results == null ? "null" : results.getErrorMessage()));
            return;
        }
        println("// " + function.getName() + " @ " + function.getEntryPoint());
        println(results.getDecompiledFunction().getC());
    }
}
