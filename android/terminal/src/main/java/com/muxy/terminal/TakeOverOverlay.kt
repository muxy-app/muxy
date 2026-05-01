package com.muxy.terminal

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.DesktopWindows
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun TakeOverOverlay(
    ownerName: String,
    foreground: Color,
    background: Color,
    onTakeOver: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(background.copy(alpha = 0.92f)),
        contentAlignment = Alignment.Center,
    ) {
        Surface(
            color = foreground.copy(alpha = 0.08f),
            contentColor = foreground,
            shape = RoundedCornerShape(20.dp),
            modifier = Modifier
                .padding(horizontal = 24.dp)
                .widthIn(max = 360.dp),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                Icon(Icons.Outlined.DesktopWindows, contentDescription = null, tint = foreground)
                Text(
                    text = "Controlled on $ownerName",
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 16.sp,
                    color = foreground,
                )
                Text(
                    text = "This terminal is currently being used on $ownerName. Take over to control it from here.",
                    fontSize = 13.sp,
                    color = foreground.copy(alpha = 0.7f),
                    textAlign = TextAlign.Center,
                )
                Spacer(Modifier.height(4.dp))
                Button(
                    onClick = onTakeOver,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = foreground,
                        contentColor = background,
                    ),
                ) {
                    Text("Take Over", fontWeight = FontWeight.SemiBold)
                }
            }
        }
    }
}
