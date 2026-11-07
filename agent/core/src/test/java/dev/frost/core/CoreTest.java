package dev.frost.core;

import static org.junit.jupiter.api.Assertions.*;

import java.io.File;
import org.junit.jupiter.api.Test;

class CoreTest {
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
    void compass() {
        assertEquals("S", Util.compass(0));
        assertEquals("W", Util.compass(90));
        assertEquals("N", Util.compass(180));
        assertEquals("E", Util.compass(270));
    }

    @Test
    void configRoundTrip() throws Exception {
        File f = File.createTempFile("frost", ".json");
        f.deleteOnExit();
        FrostState a = new FrostState();
        a.qol.no_clouds = true;
        a.hud.widgets.get(0).placed = true;
        a.hud.widgets.get(0).pos = new float[] {40f, 50f};
        Config.save(f, a);
        FrostState b = new FrostState();
        Config.load(f, b);
        assertTrue(b.qol.no_clouds);
        assertTrue(b.hud.widgets.get(0).placed);
        assertEquals(40f, b.hud.widgets.get(0).pos[0]);
        assertEquals(a.hud.widgets.size(), b.hud.widgets.size());
    }

    @Test
    void menuTogglesReportChanges() {
        final int[] changed = {0};
        FrostState st = new FrostState();
        Menu menu = new Menu(new Menu.Host() {
            public java.io.File gameDir() { return new java.io.File("build/tmp"); }
            public void settingsChanged() { changed[0]++; }
        }, st);
        Menu.Canvas canvas = new Menu.Canvas() {
            public void fill(int a, int b, int c, int d, int argb) { }
            public void text(String t, int x, int y, int argb) { }
            public int width(String t) { return t.length() * 6; }
            public int lineHeight() { return 9; }
        };
        for (Menu.Tab tab : Menu.Tab.values()) {
            menu.editing = false;
            menu.tab = tab;
            int before = changed[0];
            for (int y = 14; y < 346; y += 6) {
                for (int x = 140; x < 620; x += 12) {
                    menu.render(canvas, 640, 360, x, y, false);
                    menu.render(canvas, 640, 360, x, y, true);
                }
            }
            assertTrue(changed[0] > before, tab + " controls should react to clicks");
        }
    }
}
