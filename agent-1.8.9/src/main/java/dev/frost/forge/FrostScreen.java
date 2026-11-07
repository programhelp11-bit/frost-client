package dev.frost.forge;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.FontRenderer;
import net.minecraft.client.gui.Gui;
import net.minecraft.client.gui.GuiScreen;
import org.lwjgl.input.Keyboard;
import org.lwjgl.input.Mouse;

/** The Frost menu for 1.8.9 (Right Shift): HUD editor, gameplay and performance. */
public class FrostScreen extends GuiScreen {
    private final Menu menu = new Menu(FrostForge.HOST, FrostState.INSTANCE);

    @Override
    public void drawScreen(int mouseX, int mouseY, float partialTicks) {
        final FontRenderer font = Minecraft.getMinecraft().fontRendererObj;
        Menu.Canvas canvas = new Menu.Canvas() {
            public void fill(int x1, int y1, int x2, int y2, int argb) {
                Gui.drawRect(x1, y1, x2, y2, argb);
            }

            public void text(String s, int x, int y, int argb) {
                font.drawStringWithShadow(s, x, y, argb);
            }

            public int width(String s) {
                return font.getStringWidth(s);
            }

            public int lineHeight() {
                return font.FONT_HEIGHT;
            }
        };
        boolean down = Mouse.isButtonDown(0);
        int gs = new net.minecraft.client.gui.ScaledResolution(Minecraft.getMinecraft()).getScaleFactor();
        final float u = (float) Math.max(1, Math.min(3, Math.round(3.5 / Math.max(1, gs))));
        menu.ui = u;
        net.minecraft.client.renderer.GlStateManager.pushMatrix();
        net.minecraft.client.renderer.GlStateManager.scale(u, u, 1f);
        menu.render(canvas, Math.round(this.width / u), Math.round(this.height / u), Math.round(mouseX / u), Math.round(mouseY / u), down);
        net.minecraft.client.renderer.GlStateManager.popMatrix();
    }

    @Override
    public boolean doesGuiPauseGame() {
        return false;
    }
}
