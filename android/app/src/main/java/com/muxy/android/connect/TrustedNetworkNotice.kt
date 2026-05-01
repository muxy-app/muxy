package com.muxy.android.connect

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Lock
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import com.muxy.android.ui.theme.MuxyTheme

@Composable
fun TrustedNetworkNotice(
    onAcknowledge: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.tertiaryContainer,
        contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
    ) {
        Column(modifier = Modifier.padding(20.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = Icons.Outlined.Lock,
                    contentDescription = null,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(12.dp))
                Text(
                    text = "Use only on a trusted network",
                    style = MaterialTheme.typography.titleMedium,
                )
            }
            Spacer(Modifier.height(12.dp))
            Text(
                text = "Muxy connects to your Mac over plain WebSocket (no TLS). Pairing tokens and " +
                    "every keystroke travel in the clear. Stick to Tailscale, a private VPN, or a " +
                    "home Wi-Fi network you control.",
                style = MaterialTheme.typography.bodyMedium,
            )
            Spacer(Modifier.height(16.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.End,
            ) {
                Button(onClick = onAcknowledge) {
                    Text("Got it")
                }
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun TrustedNetworkNoticePreview() {
    MuxyTheme {
        TrustedNetworkNotice(onAcknowledge = {}, modifier = Modifier.padding(24.dp))
    }
}
