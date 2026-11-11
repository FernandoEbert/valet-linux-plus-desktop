import { createRouter, createWebHashHistory } from "vue-router";
import DashboardView from "./views/DashboardView.vue";
import LogsView from "./views/LogsView.vue";
import PathsView from "./views/PathsView.vue";
import PhpView from "./views/PhpView.vue";
import SettingsView from "./views/SettingsView.vue";
import SitesView from "./views/SitesView.vue";

export const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", name: "dashboard", component: DashboardView, meta: { title: "Dashboard" } },
    { path: "/sites", name: "sites", component: SitesView, meta: { title: "Sites" } },
    { path: "/paths", name: "paths", component: PathsView, meta: { title: "Paths" } },
    { path: "/php", name: "php", component: PhpView, meta: { title: "PHP" } },
    { path: "/logs", name: "logs", component: LogsView, meta: { title: "Logs" } },
    { path: "/settings", name: "settings", component: SettingsView, meta: { title: "Settings" } },
  ],
});
