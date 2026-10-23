// Prints every reference to the given address and the containing function.
//   -postScript FindRefsToAddress.java 0x180561050
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.symbol.Reference;

public class FindRefsToAddress extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            println("usage: FindRefsToAddress.java <address>");
            return;
        }
        Address address = currentProgram
                .getAddressFactory()
                .getDefaultAddressSpace()
                .getAddress(args[0]);
        Reference[] references = getReferencesTo(address);
        println("refs: " + references.length);
        for (Reference reference : references) {
            Function function = getFunctionContaining(reference.getFromAddress());
            println(
                    "  FROM " + reference.getFromAddress() + " type " + reference.getReferenceType()
                            + " in "
                            + (function == null
                                    ? "?"
                                    : function.getName() + " @" + function.getEntryPoint()));
        }
    }
}
