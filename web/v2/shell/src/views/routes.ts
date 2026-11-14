import {
  filterModel,
  parseFilters,
  queryClient,
  reads,
  stats,
  type Declaration,
  type Filters,
  type ListData,
  type ListPath,
  type Prefetchable,
  type TimeArg,
} from "@kartenn/sdk";
import type { Component } from "vue";
import type {
  LocationQuery,
  RouteComponent,
  RouteLocationNormalized,
  RouteParams,
} from "vue-router";
import { adminRecord, adminScreen } from "../admin/routes.ts";
import { changesRecord, changesScreen } from "../changesets/routes.ts";
import { collectionById, type Collection } from "./collections.ts";
import { carriedRead, carrierOf, rowsOf } from "./data.ts";
import { paramsOf } from "./filters.ts";

// The screens of the referential behind the entries of its space and the records (task 11;
// design 021 « Routes »): each a chunk of its own, read when its route opens, and the reads
// its route declares (`meta.queries`), which a link prefetches on intent and the guard
// awaits 100 ms at most (REQ-021-22). The frame's routes (`frame/routes.ts`) take them.

const ListView = () => import("./ListView.vue");
const RecordView = () => import("./RecordView.vue");

/** What a route of the frame needs of a screen. */
export interface ScreenRoute {
  component: () => Promise<RouteComponent>;
  /** Its properties: fixed, or of the route (vue-router gives the function the route). */
  props?: Record<string, unknown> | ((route: RouteLocationNormalized) => Record<string, unknown>);
  queries?: (params: RouteParams, time: TimeArg, query: LocationQuery) => readonly Prefetchable[];
  /**
   * What the Détail tab of the right panel shows for the route (an object of the
   * administration chosen in its list): the frame gives it the parameter `:id` of the
   * route as its property `id`, and nothing else.
   */
  detail?: Component;
}

/**
 * The declarations of the kind of a list already in the cache, read as its view reads them
 * (`carrier`, `data.ts`): the filters of its properties type by them.
 */
function cachedDeclarations(path: ListPath): Declaration[] {
  const carrier = carrierOf(path);
  if (carrier === null) return [];
  const key = carriedRead(carrier).queryKey;
  return rowsOf(queryClient.getQueryData<ListData<Declaration>>(key));
}

/**
 * The filters a list of `query` sends, as far as the cache knows the declarations; `null`
 * when a property filter of the query is not typed by them: the list would be read under
 * another key, without that filter — nothing is prefetched.
 */
export function prefetchFilters(path: ListPath, query: LocationQuery): Filters | null {
  const { filters, rejected } = parseFilters(
    filterModel(path, cachedDeclarations(path)),
    paramsOf(query),
  );
  return rejected.some((r) => r.name.startsWith("prop.") && r.reason === "unknown")
    ? null
    : filters;
}

const listOf = (path: ListPath) => (_: RouteParams, time: TimeArg, query: LocationQuery) => {
  const filters = prefetchFilters(path, query);
  return filters === null ? [] : [reads.list(path, filters as never, time)];
};

/** The list of a collection, and the declarations of its kind its view reads beside it. */
function collectionScreen(collection: Collection): ScreenRoute {
  const list = listOf(collection.path);
  return {
    component: ListView,
    props: { collection },
    queries: (params, time, query) => [...list(params, time, query), carriedRead(collection.kind)],
  };
}

/** The screen of an entry of a space of the core, by its id; none yet for the others. */
export function entryScreen(id: string): ScreenRoute | undefined {
  const collection = collectionById(id);
  if (collection) return collectionScreen(collection);
  switch (id) {
    case "home":
      return { component: () => import("./HomeView.vue"), queries: () => [stats.query()] };
    case "topology":
      return {
        component: () => import("./TopologyView.vue"),
        queries: (_, time) => [reads.list("/v1/domains", null, time)],
      };
    case "anomalies":
      return {
        component: () => import("./AnomaliesView.vue"),
        queries: (params, time, query) =>
          query.tab === "gaps"
            ? listOf("/v1/property-gaps")(params, time, query)
            : listOf("/v1/anomalies")(params, time, query),
      };
    case "properties":
      return {
        component: () => import("./PropertiesView.vue"),
        queries: () => [reads.list("/v1/property-declarations", null, null)],
      };
    default:
      return changesScreen(id) ?? adminScreen(id);
  }
}

/** The record of an object of a collection of the referential (`/prefixes/:id`). */
export function recordScreen(id: string): ScreenRoute | undefined {
  const collection = collectionById(id);
  if (!collection) return changesRecord(id) ?? adminRecord(id);
  return {
    component: RecordView,
    props: (route) => ({ collection, id: String(route.params.id) }),
    queries: (params, time) => [
      reads.object(collection.kind, String(params.id), time),
      reads.sub(collection.kind, String(params.id), "properties", null, time),
    ],
  };
}
