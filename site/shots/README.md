# Website screenshots

`site/static/img/*-{dark,light}.png` are drawn by the app's own views from
sample state (`crates/ferrite-ui/src/site_shots.rs`), by iced's software
renderer at one pixel per point. Where a shot shows a web page, the page is a
picture the real engine rendered of one of the plain demo pages in `pages/`
(or of this website), placed in the tab the way a live frame is. Nothing in a
picture is painted by hand.

## Re-rendering

1. Serve the built site and the demo pages from one origin:

   ```sh
   python3 site/build.py
   mkdir -p /tmp/shots-www && cp -r site/dist/. /tmp/shots-www/ && cp site/shots/pages/* /tmp/shots-www/
   python3 -m http.server 8080 -d /tmp/shots-www
   ```

2. Render each page with the engine at the size of the window's content area
   (1280 x 725; 1280 x 459 above the developer tools). The test checks the
   sizes and names the right one if the layout has changed. The site is shot
   11 seconds in, when its hero shows the "stopped" step.

   ```sh
   cargo build -p ferrite-servo --features servo --example page_shot
   shot() { target/debug/examples/page_shot "$1" "$2" "/tmp/shots/$3.png" 1280 "$4" 1; }
   mkdir -p /tmp/shots
   shot http://localhost:8080/          11000 site 725
   shot http://localhost:8080/news.html  8000 news 725
   shot http://localhost:8080/form.html  8000 form 725
   shot http://localhost:8080/shop.html  8000 shop 459
   ```

   `page_shot` also prints the page's console: the Console shot's six
   messages are what it reports for `shop.html`.

3. Draw the interface over them:

   ```sh
   FERRITE_SHOT_PAGES=/tmp/shots cargo test -p ferrite-ui --lib site_shots -- --ignored
   ```

Without `FERRITE_SHOT_PAGES` only the panel shots (agent, approval, runtime,
settings) are redrawn.

On Linux the engine needs Mesa's EGL (`libegl1`, `libegl-mesa0`,
`libgl1-mesa-dri`) even without a display.
