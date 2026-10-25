/* Consent gate: figure script, built by figures/ferrite-gate.mjs */
(function(){
"use strict";
const k = window.isoKit;
const D = {"P":{"origin":[320,169.31329124447944],"scale":1.6157440595630226,"azimuth":58,"elevation":30},"COLS":[-21,-15,-9,-3,3,9,15,21],"CAPS":["web.read","web.navigate","web.interact","web.download","scoped.read","clipboard.read","clipboard.write","js.execute"],"ALLOWED":[0,1,2],"START":-76,"END":76,"WAIT":24,"GATE_X":42,"GATE_H":16,"PIN_REST":16,"PIN_L":27,"PIN_R":1.3,"PIN_THROUGH":0.6,"CARD_Z":6,"CARD_TOP":6.8,"TPL_Z":12.8,"TPL_TOP":14,"BLOCK_Z":26,"CARDS":["<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M345.5 139.9L344.8 140.4L343.8 140.6L342.6 140.7L341.5 140.6L340.6 140.3L339.9 139.8L339.5 139.3L339.5 138.7L340 138.2L340.7 137.8L341.7 137.5L342.9 137.4L344 137.6L344.9 137.9L345.6 138.3L346 138.9L345.9 139.4Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M345.5 139.9L344.8 140.4L343.8 140.6L342.6 140.7L341.5 140.6L340.6 140.3L339.9 139.8L339.5 139.3L339.5 138.7L340 138.2L340.7 137.8L341.7 137.5L342.9 137.4L344 137.6L344.9 137.9L345.6 138.3L346 138.9L345.9 139.4Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M339 145.1L338.3 145.6L337.3 145.8L336.1 145.9L335 145.8L334.1 145.5L333.4 145L333 144.5L333 143.9L333.5 143.4L334.2 143L335.2 142.7L336.4 142.6L337.5 142.8L338.4 143.1L339.1 143.5L339.5 144.1L339.4 144.6Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M339 145.1L338.3 145.6L337.3 145.8L336.1 145.9L335 145.8L334.1 145.5L333.4 145L333 144.5L333 143.9L333.5 143.4L334.2 143L335.2 142.7L336.4 142.6L337.5 142.8L338.4 143.1L339.1 143.5L339.5 144.1L339.4 144.6Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M332.5 150.3L331.8 150.8L330.8 151L329.6 151.1L328.5 151L327.6 150.7L326.9 150.2L326.5 149.7L326.5 149.1L327 148.6L327.7 148.2L328.7 147.9L329.9 147.8L331 148L331.9 148.3L332.6 148.7L333 149.3L332.9 149.8Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M332.5 150.3L331.8 150.8L330.8 151L329.6 151.1L328.5 151L327.6 150.7L326.9 150.2L326.5 149.7L326.5 149.1L327 148.6L327.7 148.2L328.7 147.9L329.9 147.8L331 148L331.9 148.3L332.6 148.7L333 149.3L332.9 149.8Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M326 155.5L325.3 156L324.3 156.2L323.1 156.3L322 156.2L321.1 155.9L320.4 155.4L320 154.9L320.1 154.3L320.5 153.8L321.2 153.4L322.2 153.1L323.4 153L324.5 153.2L325.4 153.5L326.1 153.9L326.5 154.5L326.4 155Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M326 155.5L325.3 156L324.3 156.2L323.1 156.3L322 156.2L321.1 155.9L320.4 155.4L320 154.9L320.1 154.3L320.5 153.8L321.2 153.4L322.2 153.1L323.4 153L324.5 153.2L325.4 153.5L326.1 153.9L326.5 154.5L326.4 155Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M319.5 160.7L318.8 161.2L317.8 161.4L316.6 161.5L315.5 161.4L314.6 161.1L313.9 160.6L313.5 160.1L313.6 159.5L314 159L314.7 158.6L315.7 158.3L316.9 158.2L318 158.4L318.9 158.7L319.6 159.1L320 159.6L319.9 160.2Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M319.5 160.7L318.8 161.2L317.8 161.4L316.6 161.5L315.5 161.4L314.6 161.1L313.9 160.6L313.5 160.1L313.6 159.5L314 159L314.7 158.6L315.7 158.3L316.9 158.2L318 158.4L318.9 158.7L319.6 159.1L320 159.6L319.9 160.2Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M313 165.9L312.3 166.4L311.3 166.6L310.1 166.7L309 166.6L308.1 166.3L307.4 165.8L307 165.3L307.1 164.7L307.5 164.2L308.2 163.8L309.2 163.5L310.4 163.4L311.5 163.6L312.4 163.9L313.1 164.3L313.5 164.8L313.5 165.4Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M313 165.9L312.3 166.4L311.3 166.6L310.1 166.7L309 166.6L308.1 166.3L307.4 165.8L307 165.3L307.1 164.7L307.5 164.2L308.2 163.8L309.2 163.5L310.4 163.4L311.5 163.6L312.4 163.9L313.1 164.3L313.5 164.8L313.5 165.4Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"297.3\" cy=\"175.5\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M306.5 171.1L305.8 171.6L304.8 171.8L303.6 171.9L302.5 171.8L301.6 171.5L300.9 171L300.5 170.5L300.6 169.9L301 169.4L301.7 169L302.7 168.7L303.9 168.6L305 168.8L305.9 169.1L306.6 169.5L307 170L307 170.6Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M306.5 171.1L305.8 171.6L304.8 171.8L303.6 171.9L302.5 171.8L301.6 171.5L300.9 171L300.5 170.5L300.6 169.9L301 169.4L301.7 169L302.7 168.7L303.9 168.6L305 168.8L305.9 169.1L306.6 169.5L307 170L307 170.6Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>","<g class=\"iso-solid\" data-lit=\"\"><path class=\"iso-fill\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9Z\"/><path class=\"iso-shade\" data-shade=\"0\" d=\"M367.7 143.4L367.2 144L367.2 142.6L367.7 142ZM367.2 144L314.4 186.3L314.4 184.9L367.2 142.6ZM314.4 186.3L313.5 186.8L313.5 185.4L314.4 184.9Z\"/><path class=\"iso-shade\" data-shade=\"1\" d=\"M313.5 186.8L312.3 187.1L312.3 185.6L313.5 185.4ZM312.3 187.1L311.1 187.1L311.1 185.7L312.3 185.6Z\"/><path class=\"iso-shade\" data-shade=\"2\" d=\"M311.1 187.1L309.9 186.9L309.9 185.4L311.1 185.7Z\"/><path class=\"iso-shade\" data-shade=\"3\" d=\"M309.9 186.9L273.8 175.6L273.8 174.2L309.9 185.4ZM273.8 175.6L272.9 175.2L272.9 173.7L273.8 174.2ZM272.9 175.2L272.4 174.6L272.4 173.2L272.9 173.7ZM272.4 174.6L272.3 173.9L272.3 172.5L272.4 173.2Z\"/><path class=\"iso-top\" d=\"M366.2 140.4L367.1 140.8L367.6 141.4L367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1Z\"/><path class=\"iso-line iso-crease\" data-tone=\"lo\" d=\"M367.7 142L367.2 142.6L314.4 184.9L313.5 185.4L312.3 185.6L311.1 185.7L309.9 185.4L273.8 174.2L272.9 173.7L272.4 173.2L272.3 172.5\"/><path class=\"iso-line iso-edge\" data-tone=\"hi\" d=\"M272.3 172.5L272.8 171.9L325.6 129.6L326.5 129.2L327.7 128.9L328.9 128.9L330.1 129.1L366.2 140.4L367.1 140.8L367.6 141.4L367.7 142M367.7 143.4L367.2 144L314.4 186.3L313.5 186.8L312.3 187.1L311.1 187.1L309.9 186.9L273.8 175.6L272.9 175.2L272.4 174.6L272.3 173.9M367.7 142L367.7 143.4M272.3 172.5L272.3 173.9\"/></g><g class=\"iso-dots\" data-tone=\"lo\"><circle cx=\"342.7\" cy=\"139.1\" r=\"0.38\"/><circle cx=\"336.2\" cy=\"144.3\" r=\"0.38\"/><circle cx=\"329.7\" cy=\"149.5\" r=\"0.38\"/><circle cx=\"323.2\" cy=\"154.7\" r=\"0.38\"/><circle cx=\"316.8\" cy=\"159.9\" r=\"0.38\"/><circle cx=\"310.3\" cy=\"165.1\" r=\"0.38\"/><circle cx=\"303.8\" cy=\"170.3\" r=\"0.38\"/></g><path class=\"fig-hole\" d=\"M300 176.3L299.3 176.8L298.3 177L297.1 177.1L296 177L295.1 176.7L294.4 176.2L294 175.7L294.1 175.1L294.5 174.6L295.2 174.2L296.2 173.9L297.4 173.8L298.5 174L299.4 174.3L300.1 174.7L300.5 175.2L300.5 175.8Z\"/><path class=\"iso-line\" data-tone=\"hi\" d=\"M300 176.3L299.3 176.8L298.3 177L297.1 177.1L296 177L295.1 176.7L294.4 176.2L294 175.7L294.1 175.1L294.5 174.6L295.2 174.2L296.2 173.9L297.4 173.8L298.5 174L299.4 174.3L300.1 174.7L300.5 175.2L300.5 175.8Z\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M331.6 132.6L316.4 144.7\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M336.8 134.2L326 142.9\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M342 135.8L335.5 141\"/><path class=\"iso-line\" data-tone=\"lo\" data-free=\"\" d=\"M307.9 182.4L320.9 172\"/>"],"SCRIPT":[[1,"yes"],[7,"no"],[0,"yes"],[6,"yes"],[2,"yes"],[3,"no"]]};
function mount(root){

const P = D.P, COLS = D.COLS, CAPS = D.CAPS, ALLOWED = D.ALLOWED;
const stage = root.querySelector(".iso-stage");
const svg = stage.querySelector("svg");
const q = (name) => svg.querySelector('[data-part="' + name + '"]');
const car = q("carriage"), cardSlot = q("card"), slotA = q("slot-a"), slotB = q("slot-b");
const low = q("pins-low"), mid = q("pins-mid"), high = q("pins-high"), gate = q("gate"), lamp = q("lamp");
const hop = [...svg.querySelectorAll("[data-hop]")], outs = [...svg.querySelectorAll("[data-out]")];
const contacts = [...svg.querySelectorAll("[data-col]")];
const inds = [...svg.querySelectorAll("[data-ind]")];
const readout = root.querySelector("[data-readout]");
const still = matchMedia("(prefers-reduced-motion: reduce)");
const S = (paths, style) => k.solidSvg(paths, style);
const SCRIPT = D.SCRIPT;

let cx = D.START, target = D.START, gateH = 0, gateWant = 0, cardAlpha = 0;
const pins = COLS.map(() => ({ at: D.PIN_REST, rate: 0, want: D.PIN_REST }));
let phase = "load", clock = 0, step = 0, col = SCRIPT[0][0], decision = SCRIPT[0][1], ran = 0, held = 0;
let frame = 0, last = 0, visible = false, autoUntil = 0, queued = null, now = 0;
const say = (text) => { if (readout.textContent !== text) readout.textContent = text; };
const describe = (text) => stage.setAttribute("aria-valuetext", text);

function loadCard() {
  if (queued !== null) { col = queued; decision = "ask"; queued = null; }
  else { col = SCRIPT[step % SCRIPT.length][0]; decision = SCRIPT[step % SCRIPT.length][1]; step++; }
  cardSlot.innerHTML = D.CARDS[col];
  cardAlpha = 0;
  const left = 8 - (step % 8);
  hop.forEach((el, i) => el.style.opacity = i < left ? 1 : 0);
}
function pinTargets(probe) {
  COLS.forEach((_, i) => {
    let want = D.PIN_REST;
    if (probe) {
      if (!ALLOWED.includes(i)) want = D.TPL_TOP;          // stopped by the template
      else if (i === col) want = D.PIN_THROUGH;             // through template and card
      else want = D.CARD_TOP;                               // stopped by the card
    }
    pins[i].want = want;
  });
}
const allowed = () => ALLOWED.includes(col);
function drawPins() {
  let a = "", b = "", c = "";
  const order = COLS.map((y, i) => i);
  for (const i of order) {
    const y = COLS[i], bottom = pins[i].at, top = bottom + D.PIN_L;
    const tone = { tone: i === col && bottom < D.CARD_TOP - 0.2 ? "hi" : "mid", crease: "none" };
    if (bottom < D.CARD_Z - 0.01) a += S(k.cylinder(0, y, D.PIN_R, bottom, Math.min(D.CARD_Z, top) - bottom, P, 16), tone);
    const m0 = Math.max(bottom, D.CARD_TOP), m1 = Math.min(D.TPL_Z, top);
    if (m1 > m0 + 0.01) b += S(k.cylinder(0, y, D.PIN_R, m0, m1 - m0, P, 16), tone);
    const h0 = Math.max(bottom, D.TPL_TOP), h1 = Math.min(D.BLOCK_Z + 0.4, top);
    if (h1 > h0 + 0.01) c += S(k.cylinder(0, y, D.PIN_R, h0, h1 - h0, P, 16), tone);
  }
  low.innerHTML = a; mid.innerHTML = b; high.innerHTML = c;
  contacts.forEach((el, i) => el.setAttribute("data-tone", pins[i].at < D.PIN_THROUGH + 0.3 ? "green" : "faint"));
  inds.forEach((el, i) => { const through = pins[i].at < D.PIN_THROUGH + 0.3; const stopped = i === col && pins[i].want === D.TPL_TOP && pins[i].at < D.PIN_REST - 1; el.setAttribute("data-state", through ? "go" : stopped ? "stop" : ""); });
}
function drawGate() {
  if (gateH < 0.25) { gate.innerHTML = ""; return; }
  gate.innerHTML = S(k.slabOf({ x: D.GATE_X, y: -27, w: 4, d: 54, r: 0.8 }, 0, gateH, P, 3, 0.4), { tone: "mid" })
    + k.dotsSvg([k.iso([D.GATE_X + 2, -20, gateH], P), k.iso([D.GATE_X + 2, 20, gateH], P)], { size: 0.42 });
}
function draw() {
  car.setAttribute("transform", k.translateAlong(P, [cx, 0, 0]));
  const wantB = cx >= 20;
  if (wantB && car.parentNode !== slotB) slotB.appendChild(car);
  if (!wantB && car.parentNode !== slotA) slotA.appendChild(car);
  cardSlot.style.opacity = cardAlpha.toFixed(3);
  drawPins();
  drawGate();
  lamp.setAttribute("data-on", phase === "hold" || phase === "toGate" ? "1" : "0");
}
const name = () => CAPS[col];
function enter(next) { phase = next; clock = 0; }
function decide(yes) {
  if (phase !== "hold") return;
  if (yes) { enter("approved"); gateWant = 0; describe(name() + ": you approved it, it runs"); }
  else { enter("denied"); target = D.START; describe(name() + ": you denied it, nothing ran"); }
  run();
}
function tick(t) {
  now = t;
  const dt = last ? Math.min((t - last) / 1000, 1 / 30) : 1 / 60;
  last = t;
  const calm = still.matches;
  clock += dt;
  const ease = (v, w, tau) => calm ? w : v + (w - v) * (1 - Math.exp(-dt / tau));
  switch (phase) {
    case "load":
      cardAlpha = ease(cardAlpha, 1, 0.18);
      say("card " + ((ran + held) % 99 + 1) + " · " + name() + " · fed");
      if (clock > 0.7) { cardAlpha = 1; target = 0; enter("feed"); }
      break;
    case "feed":
      say(name() + " · to the reader");
      if (Math.abs(cx - target) < 0.05) { cx = target; pinTargets(true); enter("probe"); }
      break;
    case "probe":
      say(allowed() ? name() + " · in the fingerprint" : name() + " · not predicted · deviation");
      if (clock > 1.5) { pinTargets(false); enter(allowed() ? "lift" : "liftHold"); }
      break;
    case "lift":
      if (clock > 0.5) { target = D.END; enter("out"); describe(name() + ": predicted, it runs"); }
      break;
    case "liftHold":
      if (clock > 0.4) { gateWant = D.GATE_H; target = D.WAIT; enter("toGate"); }
      break;
    case "toGate":
      say(name() + " · held at the gate");
      if (Math.abs(cx - target) < 0.05 && Math.abs(gateH - gateWant) < 0.05) { cx = target; enter("hold"); held++; describe(name() + ": held, waiting for you. Enter approves, Escape denies"); }
      break;
    case "hold":
      say(name() + " · waiting for you");
      if (clock > (now > autoUntil ? 2.4 : 4.5)) decide(decision === "yes");
      break;
    case "approved":
      say(name() + " · you approved · runs");
      if (gateH < 0.25) { target = D.END; enter("out"); }
      break;
    case "denied":
      say(name() + " · denied · nothing ran");
      cardAlpha = ease(cardAlpha, 0, 0.35);
      if (Math.abs(cx - target) < 0.05) { cx = target; loadCard(); enter("load"); }
      break;
    case "out":
      if (allowed() || clock > 0) say(name() + " · ran");
      if (Math.abs(cx - target) < 0.05) {
        cx = target; ran++;
        outs.forEach((el, i) => el.style.opacity = i < Math.min(8, ran % 9) ? 1 : 0);
        cardAlpha = 0; target = D.START; enter("back");
      }
      break;
    case "back":
      say("ran " + ran + " · held " + held);
      if (Math.abs(cx - target) < 0.05) { cx = target; loadCard(); enter("load"); }
      break;
  }
  cx = ease(cx, target, 0.32);
  if (Math.abs(cx - target) < 0.04) cx = target;
  gateH = ease(gateH, gateWant, 0.22);
  if (Math.abs(gateH - gateWant) < 0.04) gateH = gateWant;
  for (const p of pins) {
    if (calm) { p.at = p.want; p.rate = 0; continue; }
    for (let s = 0; s < 4; s++) { p.rate += ((p.want - p.at) * 260 - p.rate * 15) * dt / 4; p.at += p.rate * dt / 4; }
    if (p.want > p.at && p.want - p.at < 0.004 && Math.abs(p.rate) < 0.01) { p.at = p.want; p.rate = 0; }
    // A pin can't sink into what stopped it.
    if (p.at < p.want - 0.001 && p.want < D.PIN_REST && p.rate < 0) { p.at = p.want; p.rate = -p.rate * 0.25; }
  }
  draw();
  frame = visible ? requestAnimationFrame(tick) : 0;
}
const run = () => { if (!frame && visible) { last = 0; frame = requestAnimationFrame(tick); } };
new IntersectionObserver((entries) => { visible = entries[entries.length - 1].isIntersecting; if (visible) run(); }, { rootMargin: "120px 0px" }).observe(stage);
const hush = () => { autoUntil = performance.now() + 6000; };
stage.addEventListener("click", () => { hush(); decide(true); });
stage.addEventListener("keydown", (event) => {
  if (event.key >= "1" && event.key <= "8") { hush(); queued = Number(event.key) - 1; say("next card · " + CAPS[queued]); event.preventDefault(); run(); return; }
  if (event.key === "Enter" || event.key === " ") { hush(); decide(true); event.preventDefault(); }
  if (event.key === "Escape" || event.key === "Backspace") { hush(); decide(false); event.preventDefault(); }
});
outs.forEach((el) => el.style.opacity = 0);
loadCard();
draw();

}
function start(){document.querySelectorAll('[data-figure="ferrite-gate"]').forEach(function(root){if(!root.dataset.live){root.dataset.live="1";mount(root);}});}
if(document.readyState==="loading")document.addEventListener("DOMContentLoaded",start);else start();
})();
