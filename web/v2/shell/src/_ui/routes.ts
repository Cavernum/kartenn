import type { RouteRecordRaw } from "vue-router";

/**
 * The catalogue of the design system (spec 020, design « Catalogue »; REQ-020-24): index,
 * tokens, icons and the generic page of a component, which reads its
 * `<Nom>.catalogue.ts`. Mode, density and language live in the query, so that the
 * capture of a state is a URL.
 */
export const catalogueRoutes: RouteRecordRaw[] = [
  {
    path: "/_ui",
    component: () => import("./CatalogueLayout.vue"),
    children: [
      { path: "", name: "ui-index", component: () => import("./IndexPage.vue") },
      { path: "jetons", name: "ui-tokens", component: () => import("./TokensPage.vue") },
      { path: "icones", name: "ui-icons", component: () => import("./IconsPage.vue") },
      {
        path: "c/:component",
        name: "ui-component",
        component: () => import("./ComponentPage.vue"),
        props: true,
      },
    ],
  },
];
