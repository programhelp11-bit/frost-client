package dev.frost.core;

import java.util.ArrayList;
import java.util.List;

/**
 * JSON models shared with the Rust launcher (see src/ipc.rs, src/trainer.rs, src/hud_manager.rs).
 * Field names intentionally use snake_case so Gson maps them 1:1 to serde's output.
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

        /** Used until the launcher pushes the user's layout. */
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

    public static final class LibEntry {
        public String id = "";
        public String name = "";
        public String color = "5A8CFF";
    }

    /** What the radial menus show; pushed by the launcher. */
    public static final class Library {
        public List<LibEntry> skins = new ArrayList<>();
        public List<LibEntry> capes = new ArrayList<>();
        public String active_cape = "frost";
    }

    public static final class CosmeticEntry {
        public long skin_rev;
        public boolean frost;
        public String cape_url;
        public String cape_hash;
    }

    public static final class BotConfig {
        public int id;
        public String name = "FrostBot";
        /** sword | crystal | axe_shield | mace */
        public String style = "sword";
        public float difficulty = 0.5f;
        public boolean legacy_combat;
        /** none | flat | crystal | island */
        public String arena = "none";
        public boolean kit;
    }

    public static final class BotCommand {
        public boolean forward, backward, left, right, jump, sprint;
        public float d_yaw, d_pitch;
        public boolean attack, use_item, place_crystal, hit_crystal;
    }

    public static final class Vec3 {
        public double x, y, z;

        public Vec3() {}

        public Vec3(double x, double y, double z) {
            this.x = x;
            this.y = y;
            this.z = z;
        }
    }

    public static final class Obs {
        public Vec3 bot_pos = new Vec3();
        public float bot_yaw, bot_pitch, bot_health = 20f;
        public boolean on_ground;
        public Vec3 target_pos = new Vec3();
        public Vec3 target_vel = new Vec3();
        public boolean target_blocking;
        public float target_health = 20f;
        public Float crystal_to_target;
        public float dt = 0.05f;
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

    public static final class KbProfile {
        public double strength = 0.4;
        public double friction = 2.0;
        public double vertical_cap = 0.4;
        public boolean legacy = false;
    }
}
