package dev.frost.core;

import static org.junit.jupiter.api.Assertions.*;

import com.google.gson.JsonParser;
import org.junit.jupiter.api.Test;

class CoreTest {
    @Test
    void settingsMessageFromRust() {
        String json = "{\"type\":\"settings\",\"qol\":{\"fullbright\":true,\"minimal_viewmodel\":false,"
            + "\"item_physics\":false,\"dynamic_fov\":true,\"zoom_enabled\":true,\"zoom_key\":\"Z\",\"zoom_fov\":40.0,"
            + "\"show_coordinates\":true,\"show_biome\":false,\"hitboxes\":true,\"fps_boost\":true,"
            + "\"chunk_optimizer\":true,\"chunk_render_cap\":10},"
            + "\"hud\":{\"widgets\":[{\"kind\":\"cps\",\"enabled\":true,\"pos\":[20.0,30.5],\"scale\":1.5,\"opacity\":120}],"
            + "\"snap_grid\":4.0}}";
        FrostState s = new FrostState();
        Dispatcher.handle(s, JsonParser.parseString(json).getAsJsonObject());
        assertTrue(s.qol.fullbright);
        assertEquals("Z", s.qol.zoom_key);
        assertEquals(10, s.qol.chunk_render_cap);
        assertEquals(1, s.hud.widgets.size());
        assertEquals("cps", s.hud.widgets.get(0).kind);
        assertEquals(30.5f, s.hud.widgets.get(0).pos[1]);
        assertEquals(1.5f, s.hud.widgets.get(0).scale);
    }

    @Test
    void botMessages() {
        FrostState s = new FrostState();
        Dispatcher.handle(s, JsonParser.parseString(
            "{\"type\":\"spawn_bot\",\"bot\":{\"id\":7,\"name\":\"B\",\"style\":\"axe_shield\",\"difficulty\":0.8,\"legacy_combat\":false}}")
            .getAsJsonObject());
        Models.BotConfig cfg = s.spawnQueue.poll();
        assertNotNull(cfg);
        assertEquals(7, cfg.id);
        assertEquals("axe_shield", cfg.style);

        Dispatcher.handle(s, JsonParser.parseString(
            "{\"type\":\"bot_command\",\"id\":7,\"command\":{\"forward\":true,\"backward\":false,\"left\":false,"
                + "\"right\":true,\"jump\":false,\"sprint\":true,\"d_yaw\":-12.5,\"d_pitch\":1.0,\"attack\":true,"
                + "\"use_item\":false,\"place_crystal\":false,\"hit_crystal\":false}}").getAsJsonObject());
        Models.BotCommand c = s.botCommands.get(7);
        assertTrue(c.forward && c.sprint && c.attack && c.right);
        assertEquals(-12.5f, c.d_yaw);

        Dispatcher.handle(s, JsonParser.parseString("{\"type\":\"clear_bots\"}").getAsJsonObject());
        assertTrue(s.clearBots.get());
    }

    @Test
    void cosmeticsMessage() {
        FrostState s = new FrostState();
        Dispatcher.handle(s, JsonParser.parseString(
            "{\"type\":\"cosmetics\",\"players\":{\"abc\":{\"frost\":true,\"cape_url\":null,\"cape_hash\":null},"
                + "\"def\":{\"frost\":false,\"cape_url\":null,\"cape_hash\":null}}}").getAsJsonObject());
        assertTrue(s.isFrost("abc"));
        assertFalse(s.isFrost("def"));
        assertFalse(s.isFrost("zzz"));
    }

    @Test
    void outgoingStatsShapeMatchesRust() {
        Models.Stats st = new Models.Stats();
        st.fps = 100;
        com.google.gson.JsonObject o = Dispatcher.gson().toJsonTree(st).getAsJsonObject();
        assertEquals(100, o.get("fps").getAsInt());
        assertTrue(o.has("keys") && o.has("armor") && o.has("potions") && o.has("pos"));
        assertFalse(o.has("last_reach")); // null -> omitted -> Option::None in serde
    }

    @Test
    void cpsWindowSlides() {
        Cps c = new Cps();
        long t0 = 10_000_000_000L;
        for (int i = 0; i < 10; i++) c.click(t0 + i * 50_000_000L);
        assertEquals(10, c.count(t0 + 500_000_000L));
        assertEquals(4, c.count(t0 + 1_300_000_000L));
        assertEquals(0, c.count(t0 + 3_000_000_000L));
    }

    @Test
    void compassMatchesRust() {
        assertEquals("S", Util.compass(0));
        assertEquals("W", Util.compass(90));
        assertEquals("N", Util.compass(180));
        assertEquals("E", Util.compass(270));
        assertEquals("E", Util.compass(-90));
    }

    @Test
    void knockbackMatchesRustTests() {
        Models.KbProfile modern = new Models.KbProfile();
        Models.KbProfile legacy = new Models.KbProfile();
        legacy.legacy = true;
        double[] walk = Knockback.apply(modern, 0, 0, 0, true, 0, 1, 0f, false, 0);
        double[] sprint = Knockback.apply(modern, 0, 0, 0, true, 0, 1, 0f, true, 0);
        assertTrue(Math.hypot(sprint[0], sprint[2]) > Math.hypot(walk[0], walk[2]));
        assertTrue(walk[2] > 0);
        double[] air = Knockback.apply(modern, 0, 0, 0, false, 0, 1, 0f, false, 0);
        double[] airLegacy = Knockback.apply(legacy, 0, 0, 0, false, 0, 1, 0f, false, 0);
        assertEquals(0.0, air[1]);
        assertTrue(airLegacy[1] > 0);
    }

    @Test
    void menuDragMovesWidgetAndReportsChange() {
        final int[] changed = {0};
        FrostState st = new FrostState();
        Menu.Host host = new Menu.Host() {
            public void settingsChanged() { changed[0]++; }
            public void requestBot(String style, float d, String arena, boolean kit) { }
            public void clearBots() { }
            public boolean launcherConnected() { return false; }
        };
        Menu menu = new Menu(host, st);
        Menu.Canvas canvas = new Menu.Canvas() {
            public void fill(int a, int b, int c, int d, int argb) { }
            public void text(String t, int x, int y, int argb) { }
            public int width(String t) { return t.length() * 6; }
            public int lineHeight() { return 9; }
        };
        st.bounds.put("fps", new int[] {200, 100, 50, 20});
        for (Models.Widget w : st.hud.widgets) if (w.kind.equals("fps")) w.pos = new float[] {200f, 100f};
        menu.render(canvas, 640, 360, 210, 110, false);
        menu.render(canvas, 640, 360, 210, 110, true);   // press on the widget
        menu.render(canvas, 640, 360, 250, 150, true);   // drag
        menu.render(canvas, 640, 360, 250, 150, false);  // release
        Models.Widget fps = null;
        for (Models.Widget w : st.hud.widgets) if (w.kind.equals("fps")) fps = w;
        assertEquals(240f, fps.pos[0]);
        assertEquals(140f, fps.pos[1]);
        assertEquals(1, changed[0]);
    }

    @Test
    void arenaPlansHaveFloorsAndClearAir() {
        for (String kind : new String[] {"flat", "crystal", "island"}) {
            java.util.List<ArenaPlan.Cell> cells = ArenaPlan.build(kind);
            assertTrue(cells.size() > 1000, kind);
            int[] player = ArenaPlan.playerSpot(kind);
            boolean floorUnderPlayer = false;
            for (ArenaPlan.Cell c : cells) {
                if (c.dx == player[0] && c.dz == player[2] && c.dy == 0 && c.mat != ArenaPlan.Mat.AIR) floorUnderPlayer = true;
                // the spot the player stands on must be open air
                if (c.dx == player[0] && c.dz == player[2] && c.dy == player[1]) assertEquals(ArenaPlan.Mat.AIR, c.mat);
            }
            assertTrue(floorUnderPlayer, kind);
        }
        assertEquals(0, ArenaPlan.build("none").size());
    }
}
