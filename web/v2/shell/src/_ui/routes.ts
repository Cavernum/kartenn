import type { RouteRecordRaw } from "vue-router";

/**
 * The catalogue of the design system (spec 020, design « Catalogue »; REQ-020-24): index,
 * tokens, icons, the generic page of a component, which reads its `<Nom>.catalogue.ts`, the
 * review page of the mock-ups; and the mock-ups themselves (design « Maquettes »), outside
 * the frame of the catalogue since they take the whole page. Mode, density and language
 * live in the query, so that the capture of a state is a URL.
 */
export const catalogueRoutes: RouteRecordRaw[] = [
  {
    path: "/_ui",
    component: () => import("./CatalogueLayout.vue"),
    children: [
      { path: "", name: "ui-index", component: () => import("./IndexPage.vue") },
      { path: "jetons", name: "ui-tokens", component: () => import("./TokensPage.vue") },
      { path: "icones", name: "ui-icons", component: () => import("./IconsPage.vue") },
      { path: "revue", name: "ui-review", component: () => import("./RevuePage.vue") },
      {
        path: "c/:component",
        name: "ui-component",
        component: () => import("./ComponentPage.vue"),
        props: true,
      },
    ],
  },
  {
    path: "/_ui/ecrans/:ecran",
    name: "ui-mockup",
    component: () => import("./ecrans/EcranPage.vue"),
    props: true,
  },
];
