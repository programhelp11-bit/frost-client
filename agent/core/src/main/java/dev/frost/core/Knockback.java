package dev.frost.core;

/** Port of trainer::apply_knockback (vanilla knockBack + sprint/enchant bonus). */
public final class Knockback {
    private Knockback() {}

    /**
     * @param dx,dz attacker -> victim direction (horizontal)
     * @return new victim velocity {x, y, z}
     */
    public static double[] apply(Models.KbProfile p, double vx, double vy, double vz, boolean victimOnGround,
                                 double dx, double dz, float attackerYawDeg, boolean sprinting, int kbEnchant) {
        double dist = Math.max(Math.sqrt(dx * dx + dz * dz), 1e-4);
        double x = vx / p.friction + dx / dist * p.strength;
        double z = vz / p.friction + dz / dist * p.strength;
        double y = vy;
        if (p.legacy || victimOnGround) {
            y = vy / p.friction + p.strength;
            if (y > p.vertical_cap) y = p.vertical_cap;
        }
        int extra = kbEnchant + (sprinting ? 1 : 0);
        if (extra > 0) {
            double yaw = Math.toRadians(attackerYawDeg);
            x += -Math.sin(yaw) * extra * 0.5;
            y += 0.1;
            z += Math.cos(yaw) * extra * 0.5;
        }
        return new double[] {x, y, z};
    }
}
