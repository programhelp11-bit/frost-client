package dev.frost.core;

import java.util.ArrayList;
import java.util.List;

/**
 * Settings and HUD models (stored in config/frost-client.json).
 */
public final class Models {
    private Models() {}

    public static final class Qol {
        public boolean fullbright = false;
        public boolean minimal_viewmodel = false;
        public boolean item_physics = false;
        public boolean dynamic_fov = true;
        public boolean zoom_enabled = true;
        public String zoom_key = "C";
        public float zoom_fov = 30f;
        public boolean show_coordinates = true;
        public boolean show_biome = true;
        public boolean hitboxes = false;
        public boolean fps_boost = true;
        public boolean chunk_optimizer = true;
        public int chunk_render_cap = 12;
        public boolean no_clouds = false;
        public boolean no_entity_shadows = true;
        public boolean low_biome_blend = true;
        public boolean cap_sim_distance = true;
        public int sim_distance_cap = 8;
        public boolean no_menu_blur = true;
        public boolean fast_graphics = true;
    }

    public static final class Widget {
        public String kind = "";
        public boolean enabled = false;
        public float[] pos = new float[] {0f, 0f};
        public float scale = 1f;
        public int opacity = 120;
        /** True after the player placed it in the HUD editor; otherwise the HUD stacks it automatically. */
        public boolean placed = false;

        public Widget() {}

        public Widget(String kind, boolean enabled, float x, float y) {
            this.kind = kind;
            this.enabled = enabled;
            this.pos = new float[] {x, y};
        }
    }

    public static final class HudLayout {
        public List<Widget> widgets = new ArrayList<>();
        public float snap_grid = 4f;

        /** Adds widgets a newer version knows about but the saved file does not. */
        public HudLayout merged() {
            HudLayout d = defaults();
            for (Widget w : d.widgets) {
                boolean have = false;
                for (Widget x : widgets) if (x.kind.equals(w.kind)) have = true;
                if (!have) widgets.add(w);
            }
            return this;
        }

        /** Used until the player changes the layout. */
        public static HudLayout defaults() {
            HudLayout l = new HudLayout();
            String[] kinds = {"cps", "fps", "ping", "keystrokes", "armor_status", "potion_status",
                "direction", "reach_display", "coordinates", "bw_resources", "bw_blocks", "bridge_speed"};
            boolean[] on = {true, true, true, true, false, false, false, false, false, false, false, false};
            float y = 6f;
            for (int i = 0; i < kinds.length; i++) {
                l.widgets.add(new Widget(kinds[i], on[i], 6f, y));
                y += 24f;
            }
            return l;
        }
    }

    public static final class Keys {
        public boolean w, a, s, d, space, lmb, rmb;
    }

    public static final class Armor {
        public int slot;
        public String name = "";
        public float durability_pct;
    }

    public static final class Potion {
        public String name = "";
        public int amplifier;
        public int remaining_secs;
    }

    public static final class Stats {
        public int fps, ping_ms, cps_left, cps_right;
        public Keys keys = new Keys();
        public List<Armor> armor = new ArrayList<>();
        public List<Potion> potions = new ArrayList<>();
        public float yaw;
        public Float last_reach;
        public double[] pos = new double[] {0, 0, 0};
        public String biome = "";
        // Bedwars
        public int bw_iron, bw_gold, bw_diamond, bw_emerald, bw_blocks;
        public float bridge_bps, bridge_best;
    }
}
