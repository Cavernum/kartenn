import type { components } from "@kartenn/sdk";

/** An open anomaly, as `GET /v1/anomalies` lists it (006, REQ-002-36). */
export type Anomaly = components["schemas"]["AnomalyDto"];
