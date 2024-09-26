import Task from "@divviup/dap";

const main = async () => {
  const task = new Task({
    type: "sum",
    bits: 8,
    id: "8TuT5Z5fAuutsX9DZWSqkUw6pzDl96d3tdsDJgWH2VY",
    leader: "http://localhost:8787/v09",
    helper: "http://localhost:8788/v09",
    timePrecisionSeconds: 3600,
  });

  try {
    await task.sendMeasurement(42);

    console.log("DAP report: sent")
  } catch (_) {
    console.log("DAP report: failed")
  } finally {
    console.log(`task id: ${task.id}`)
  }
}

main()
