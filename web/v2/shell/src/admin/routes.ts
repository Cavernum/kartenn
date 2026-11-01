import { reads } from "@kartenn/sdk";
import { defineAsyncComponent } from "vue";
import type { ScreenRoute } from "../views/routes.ts";

// The screens of the space `admin` (design 021 « Administration », « Routes »; spec 025,
// REQ-025-12; maquette E7): each entry a list, its object chosen by the URL
// (`/admin/groups/:id`, `/admin/roles/:id`), the same screen, shown in the Détail tab of
// the right panel (`meta.detail`, read by the frame). Groups and roles here (task 15a); the
// identities, the service accounts, the issuers (15b) and the audit (15c) come the same
// way: an entry in `adminScreen`, a record in `adminRecord` and `ADMIN_RECORDS`.

const GroupsView = () => import("./GroupsView.vue");
const RolesView = () => import("./RolesView.vue");
const GroupDetail = defineAsyncComponent(() => import("./GroupDetail.vue"));
const RoleDetail = defineAsyncComponent(() => import("./RoleDetail.vue"));

/** The entries of the space whose objects open in the right panel. */
export const ADMIN_RECORDS = ["/admin/groups", "/admin/roles"] as const;

/** The screen of an entry of the space `admin`, by its id. */
export function adminScreen(id: string): ScreenRoute | undefined {
  switch (id) {
    case "groups":
      return { component: GroupsView, queries: () => [reads.list("/v1/groups")] };
    case "roles":
      return { component: RolesView, queries: () => [reads.list("/v1/roles")] };
    default:
      return undefined;
  }
}

/** The record of an object of the space: the list, the object in the Détail tab. */
export function adminRecord(id: string): ScreenRoute | undefined {
  switch (id) {
    case "groups":
      return {
        component: GroupsView,
        props: (route) => ({ selected: String(route.params.id) }),
        detail: GroupDetail,
        queries: (params) => [
          reads.list("/v1/groups"),
          reads.read("/v1/groups/{id}", { params: { id: String(params.id) } }),
          reads.list("/v1/groups/{id}/members", null, null, { id: String(params.id) }),
        ],
      };
    case "roles":
      return {
        component: RolesView,
        props: (route) => ({ selected: String(route.params.id) }),
        detail: RoleDetail,
        queries: () => [reads.list("/v1/roles")],
      };
    default:
      return undefined;
  }
}
