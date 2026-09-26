import { mount } from "svelte";
import "@fontsource/montserrat/500.css";
import "@fontsource/montserrat/700.css";
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./app.css";
import App from "./App.svelte";

export default mount(App, { target: document.getElementById("app")! });
