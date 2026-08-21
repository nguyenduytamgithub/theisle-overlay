import { mount } from "svelte";
import "../app.css";
import { installGlobalErrorLog } from "../lib/errlog";
import App from "./App.svelte";

installGlobalErrorLog("main");

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
