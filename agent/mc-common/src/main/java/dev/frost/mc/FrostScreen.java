package dev.frost.mc;

import dev.frost.core.FrostState;
import dev.frost.core.Menu;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphics;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;
import org.lwjgl.glfw.GLFW;

/** In-game Frost menu: HUD editor, trainer and quality settings (logic lives in core/Menu). */
public class FrostScreen extends Screen {
    private final Menu menu = new Menu(FrostMc.HOST, FrostState.INSTANCE);

    public FrostScreen() {
        super(Component.literal("Frost"));
    }

    public void render(GuiGraphics g, int mouseX, int mouseY, float delta) {
        Minecraft mc = Minecraft.getInstance();
        final Font font = mc.font;
        final GuiGraphics gg = g;
        long window = GLFW.glfwGetCurrentContext();
        boolean down = window != 0L && GLFW.glfwGetMouseButton(window, GLFW.GLFW_MOUSE_BUTTON_LEFT) == GLFW.GLFW_PRESS;
        menu.render(new Menu.Canvas() {
            public void fill(int x1, int y1, int x2, int y2, int argb) {
                gg.fill(x1, y1, x2, y2, argb);
            }

            public void text(String s, int x, int y, int argb) {
                gg.drawString(font, s, x, y, argb);
            }

            public int width(String s) {
                return font.width(s);
            }

            public int lineHeight() {
                return font.lineHeight;
            }
        }, this.width, this.height, mouseX, mouseY, down);
    }

    public boolean isPauseScreen() {
        return false;
    }
}
