import { createApp } from "vue";
import App from "@/App.vue";
import "@/styles.css";

const root = document.getElementById("root");
if (root) createApp(App).mount(root);
