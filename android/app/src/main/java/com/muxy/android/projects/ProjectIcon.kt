package com.muxy.android.projects

import android.graphics.BitmapFactory
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.muxy.protocol.dto.ProjectDTO
import com.muxy.protocol.dto.ProjectIconColor

@Composable
fun ProjectIcon(
    project: ProjectDTO,
    logoBytes: ByteArray?,
    modifier: Modifier = Modifier,
    size: Dp = 40.dp,
) {
    val cornerRadius = size * 0.22f
    val initial = project.name.firstOrNull()?.uppercaseChar()?.toString().orEmpty()

    if (logoBytes != null) {
        val imageBitmap =
            remember(logoBytes) {
                BitmapFactory.decodeByteArray(logoBytes, 0, logoBytes.size)?.asImageBitmap()
            }
        if (imageBitmap != null) {
            Image(
                bitmap = imageBitmap,
                contentDescription = project.name,
                modifier =
                    modifier
                        .size(size)
                        .clip(RoundedCornerShape(cornerRadius)),
            )
            return
        }
    }

    val swatch = ProjectIconColor.swatch(forIdentifier = project.iconColor)
    if (swatch != null) {
        val rgb = ProjectIconColor.rgb(fromHex = swatch.hex)
        if (rgb != null) {
            val fill = Color(red = rgb.first.toFloat(), green = rgb.second.toFloat(), blue = rgb.third.toFloat())
            Box(
                modifier =
                    modifier
                        .size(size)
                        .background(fill, RoundedCornerShape(cornerRadius)),
                contentAlignment = Alignment.Center,
            ) {
                Text(
                    text = initial,
                    color = if (swatch.prefersDarkForeground) Color.Black else Color.White,
                    fontWeight = FontWeight.Bold,
                    fontSize = (size.value * 0.4f).sp,
                )
            }
            return
        }
    }

    Box(
        modifier =
            modifier
                .size(size)
                .background(
                    MaterialTheme.colorScheme.primary.copy(alpha = 0.15f),
                    RoundedCornerShape(cornerRadius),
                ),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = initial,
            color = MaterialTheme.colorScheme.primary,
            fontWeight = FontWeight.Bold,
            fontSize = (size.value * 0.4f).sp,
        )
    }
}
