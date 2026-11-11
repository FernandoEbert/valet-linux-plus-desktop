import { createRouter, createWebHashHistory } from "vue-router";
import DashboardView from "./views/DashboardView.vue";
import SitesView from "./views/SitesView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dashboard", component: DashboardView, meta: { title: "Dashboard" } },
    { path: "/sites", name: "sites", component: SitesView, meta: { title: "Sites" } },
  ],
});
