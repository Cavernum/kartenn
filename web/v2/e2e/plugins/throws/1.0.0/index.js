// Test plugin (spec 021, task 16): throws while its module is evaluated, before it defines
// anything; the journeys tolerate this one exception (fixtures/journeys.ts).
throw new Error("e2e plugin throws while loading");
