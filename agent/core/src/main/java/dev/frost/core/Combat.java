package dev.frost.core;

/** Vanilla-accurate damage maths for the trainer bots (pure Java, unit tested). */
public final class Combat {
    private Combat() {}

    /** Damage the bot's weapon does before armor. */
    public static float weaponDamage(String style, boolean legacy) {
        if ("axe_shield".equals(style)) return legacy ? 7f : 10f;
        if ("mace".equals(style)) return 8f;
        if ("bedwars_rush".equals(style)) return 6f;
        return legacy ? 7f : 8f;
    }

    /** Armor points of the bot's gear. */
    public static float armorPoints(String style) {
        return "bedwars_rush".equals(style) ? 14f : 20f;
    }

    public static float armorToughness(String style, boolean legacy) {
        return ("bedwars_rush".equals(style) || legacy) ? 0f : 12f;
    }

    /** Damage left after armor. Legacy (1.8) uses 4% per armor point, modern uses armor + toughness. */
    public static float afterArmor(float dmg, float armor, float toughness, boolean legacy) {
        if (legacy) return dmg * (1f - Math.min(20f, armor) * 0.04f);
        float red = Math.min(20f, Math.max(armor / 5f, armor - dmg / (2f + toughness / 4f)));
        return dmg * (1f - red / 25f);
    }

    /** Critical hit: falling, not on ground, not sprinting (modern) / not on ground and falling (legacy). */
    public static boolean isCrit(boolean onGround, double velY, boolean sprinting, boolean legacy) {
        if (onGround || velY >= 0) return false;
        return legacy || !sprinting;
    }

    /** Health after taking a hit and its damage (never below zero). */
    public static float hurt(float hp, float dmg) {
        return Math.max(0f, hp - dmg);
    }

    /** Natural regeneration per second (about one heart every 4 s, like a fed player). */
    public static float regenPerSecond() {
        return 0.5f;
    }
}
