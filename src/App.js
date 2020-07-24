import React, { Component } from 'react'
import { Grommet, Stack, Text, Box, Button } from 'grommet';
import { Github, ChatOption, Twitter, MailOption } from 'grommet-icons';
import Terminal from 'react-console-emulator'

const commands = {
  echo: {
    description: 'Echo a passed string.',
    usage: 'echo <string>',
    fn: function ()
    {
      return `${Array.from(arguments).join(' ')}`
    }
  }
}

export default class App extends Component
{
  render ()
  {
    return (
      <Grommet themeMode="dark" full>
        <Stack anchor="center" fill>
          <Terminal
            contentStyle={{ 'overflow-x': 'hidden', 'word-wrap': 'break-word' }}
            style={{ minHeight: '100%', minWidth: '100%', borderRadius: '0' }}
            commands={commands}
            promptLabel={'> '}
          />
          <Box border={{ color: 'brand', size: 'large', style: 'double', side: 'horizontal' }} pad="medium" alignContent='center'>
            <Text size='xxlarge' color='darkgrey' textAlign='center' margin='small'>
              Kyle Martin
            </Text>
            <Text size='xxlarge' color='darkgrey' textAlign='center' margin='small'>
              Computer Engineer
            </Text>
            <Text size='xxlarge' color='darkgrey' textAlign='center' margin='small'>
            </Text>

            <Box alignSelf='center' direction='row' gap='medium'>
              <Button icon={<Github size='large' />} href='https://github.com/KyleMiles' plain={true} focusIndicator={false} />
              <Button icon={<ChatOption size='large' />} href='https://app.element.io/#/user/@elykdeer:elyk.rocks' plain={true} focusIndicator={false} />
              <Button icon={<Twitter size='large' />} href='https://twitter.com/elykdeer' plain={true} focusIndicator={false} />
              <Button icon={<MailOption size='large' />} href='mailto:martinrkyle@gmail.com' plain={true} focusIndicator={false} />
            </Box>
          </Box>
        </Stack>

      </Grommet >
    )
  }
}
