import { reads } from "@kartenn/sdk";
import { defineAsyncComponent } from "vue";
import type { ScreenRoute } from "../views/routes.ts";

// The screens of the space `admin` (design 021 « Administration », « Routes »; spec 025,
// REQ-025-12; maquette E7): each entry a list, its object chosen by the URL
// (`/admin/groups/:id`, `/admin/roles/:id`), the same screen, shown in the Détail tab of
// the right panel (`meta.detail`, read by the frame). Groups and roles (task 15a); the
// identities, the service accounts and the issuers (15b, the issuers a list alone: an
// attachment has no record); the audit (15c) comes the same way: an entry in
// `adminScreen`, a record in `adminRecord` and `ADMIN_RECORDS`.

const GroupsView = () => import("./GroupsView.vue");
const RolesView = () => import("./RolesView.vue");
const IdentitiesView = () => import("./IdentitiesView.vue");
const AccountsView = () => import("./AccountsView.vue");
const GroupDetail = defineAsyncComponent(() => import("./GroupDetail.vue"));
const RoleDetail = defineAsyncComponent(() => import("./RoleDetail.vue"));
const IdentityDetail = defineAsyncComponent(() => import("./IdentityDetail.vue"));
const AccountDetail = defineAsyncComponent(() => import("./AccountDetail.vue"));

/** The entries of the space whose objects open in the right panel. */
export const ADMIN_RECORDS = [
  "/admin/groups",
  "/admin/roles",
  "/admin/identities",
  "/admin/accounts",
] as const;

/** The screen of an entry of the space `admin`, by its id. */
export function adminScreen(id: string): ScreenRoute | undefined {
  switch (id) {
    case "groups":
      return { component: GroupsView, queries: () => [reads.list("/v1/groups")] };
    case "roles":
      return { component: RolesView, queries: () => [reads.list("/v1/roles")] };
    case "identities":
      return { component: IdentitiesView, queries: () => [reads.list("/v1/identities")] };
    case "accounts":
      return { component: AccountsView, queries: () => [reads.list("/v1/service-accounts")] };
    case "issuers":
      return {
        component: () => import("./IssuersView.vue"),
        queries: () => [reads.list("/v1/issuer-attachments")],
      };
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
    case "identities":
      return {
        component: IdentitiesView,
        props: (route) => ({ selected: String(route.params.id) }),
        detail: IdentityDetail,
        queries: (params) => [
          reads.list("/v1/identities"),
          reads.read("/v1/identities/{id}", { params: { id: String(params.id) } }),
        ],
      };
    case "accounts":
      return {
        component: AccountsView,
        props: (route) => ({ selected: String(route.params.id) }),
        detail: AccountDetail,
        queries: (params) => [
          reads.list("/v1/service-accounts"),
          reads.read("/v1/service-accounts/{id}", { params: { id: String(params.id) } }),
        ],
      };
    default:
      return undefined;
  }
}
