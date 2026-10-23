// Prints defined strings containing the needle and every function that
// references them.
//   analyzeHeadless <proj> mngbase -process mngbase.dll -noanalysis \
//     -scriptPath tools\rea\ghidra -postScript FindStringRefs.java ChoiceParameterLinks
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.Reference;

public class FindStringRefs extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            println("usage: FindStringRefs.java <needle>");
            return;
        }
        String needle = args[0];
        Listing listing = currentProgram.getListing();
        int found = 0;
        for (Data data : listing.getDefinedData(true)) {
            Object value = data.getValue();
            if (!(value instanceof String text) || !text.contains(needle)) {
                continue;
            }
            found++;
            println("STRING " + data.getAddress() + " : " + text);
            for (Reference reference : getReferencesTo(data.getAddress())) {
                Function function = getFunctionContaining(reference.getFromAddress());
                println(
                        "  REF " + reference.getFromAddress() + " in "
                                + (function == null
                                        ? "?"
                                        : function.getName() + " @" + function.getEntryPoint()));
            }
        }
        println("matches: " + found);
    }
}
