// Test plugin (spec 007, task 15): throws while its module is evaluated, before it defines
// anything; the journeys tolerate this one exception (fixtures/index.ts).
throw new Error("e2e plugin throws while loading");
