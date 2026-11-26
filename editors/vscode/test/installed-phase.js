const fs = require("node:fs");
const path = require("node:path");
exports.phase = (name, details = {}) => {
  const line = JSON.stringify({
    time: new Date().toISOString(),
    phase: name,
    ...details,
  });
  console.log(line);
  if (process.env.IBCMD_ACCEPTANCE_RUN) {
    fs.appendFileSync(
      path.join(process.env.IBCMD_ACCEPTANCE_RUN, "phases.jsonl"),
      line + "\n",
    );
  }
};
