package dev.frost.core;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.google.gson.reflect.TypeToken;

import java.util.Collections;
import java.util.HashMap;
import java.util.Map;

/** Applies launcher -> game messages (ToGame in src/ipc.rs) to {@link FrostState}. */
public final class Dispatcher {
    private static final Gson GSON = new Gson();
    private static final java.lang.reflect.Type PLAYERS =
        new TypeToken<HashMap<String, Models.CosmeticEntry>>() {}.getType();

    private Dispatcher() {}

    public static void handle(FrostState state, JsonObject msg) {
        if (!msg.has("type")) return;
        String type = msg.get("type").getAsString();
        if (type.equals("settings")) {
            if (msg.has("qol")) state.qol = GSON.fromJson(msg.get("qol"), Models.Qol.class);
            if (msg.has("hud")) state.hud = GSON.fromJson(msg.get("hud"), Models.HudLayout.class);
        } else if (type.equals("cosmetics")) {
            Map<String, Models.CosmeticEntry> players = GSON.fromJson(msg.get("players"), PLAYERS);
            state.cosmetics = players == null ? Collections.<String, Models.CosmeticEntry>emptyMap() : players;
        } else if (type.equals("library")) {
            Models.Library lib = GSON.fromJson(msg, Models.Library.class);
            if (lib != null) state.library = lib;
        } else if (type.equals("own_cape")) {
            String b64 = msg.has("png_b64") && !msg.get("png_b64").isJsonNull() ? msg.get("png_b64").getAsString() : null;
            state.ownCapePng = b64 == null ? null : java.util.Base64.getDecoder().decode(b64);
        } else if (type.equals("bot_policy")) {
            com.google.gson.JsonArray arr = msg.getAsJsonArray("params");
            float[] p = new float[arr.size()];
            for (int i = 0; i < p.length; i++) p[i] = arr.get(i).getAsFloat();
            state.botParams = p;
            state.botGeneration = msg.has("generation") ? msg.get("generation").getAsInt() : 0;
        } else if (type.equals("notice")) {
            if (msg.has("text")) state.notify(msg.get("text").getAsString());
        } else if (type.equals("spawn_bot")) {
            state.spawnQueue.add(GSON.fromJson(msg.get("bot"), Models.BotConfig.class));
        } else if (type.equals("clear_bots")) {
            state.clearBots.set(true);
        } else if (type.equals("knockback_trainer")) {
            state.knockbackTrainer = msg.get("enabled").getAsBoolean();
            if (msg.has("profile")) state.kbProfile = GSON.fromJson(msg.get("profile"), Models.KbProfile.class);
        } else if (type.equals("bot_command")) {
            state.botCommands.put(msg.get("id").getAsInt(), GSON.fromJson(msg.get("command"), Models.BotCommand.class));
        } else if (type.equals("bedwars")) {
            state.bedwarsBridgeTrainer = msg.has("bridge_trainer") && msg.get("bridge_trainer").getAsBoolean();
        }
    }

    public static Gson gson() {
        return GSON;
    }
}
