import { mount } from "svelte"
import "../src/styles/app.css"
import Widget from "./Widget.svelte"

mount(Widget, { target: document.getElementById("app")! })
