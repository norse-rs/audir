/*
 * Copyright (c) 2007-2011 The Khronos Group Inc.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy of
 * this software and /or associated documentation files (the "Materials "), to
 * deal in the Materials without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Materials, and to permit persons to whom the Materials are
 * furnished to do so, subject to
 * the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Materials.
 *
 * THE MATERIALS ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE MATERIALS OR THE USE OR OTHER DEALINGS IN THE
 * MATERIALS.
 *
 * OpenSLES_Platform.h - OpenSL ES version 1.1
 *
 */

/****************************************************************************/
/* NOTE: This file contains definitions for the base types and the          */
/* SLAPIENTRY macro. This file **WILL NEED TO BE EDITED** to provide        */
/* the correct definitions specific to the platform being used.             */
/****************************************************************************/

#ifndef _OPENSLES_PLATFORM_H_
#define _OPENSLES_PLATFORM_H_

typedef char                        sl_char_t;
typedef unsigned char               sl_uint8_t;
typedef signed char                 sl_int8_t;
typedef unsigned short              sl_uint16_t;
typedef signed short                sl_int16_t;
typedef unsigned long               sl_uint32_t;
typedef signed long                 sl_int32_t;
typedef float                       sl_float32_t;
typedef double                      sl_float64_t;

/****************************************************************************/
/* NOTE: SL_BYTEORDER_NATIVEBIGENDIAN will cause SL_BYTEORDER_NATIVE to     */
/* mirror SL_BYTEORDER_BIGENDIAN, otherwise it will default to              */
/* SL_BYTEORDER_LITTLEENDIAN.                                               */
/****************************************************************************/
//#define SL_BYTEORDER_NATIVEBIGENDIAN  1

/** SLAPIENTRY is a system-dependent API function prototype declaration macro.
*
* Example:
* #ifdef WIN32
* # define SLAPIENTRY __stdcall
* #endif
*/
#ifndef SLAPIENTRY
#define SLAPIENTRY                 /* override per-platform */
#endif

/** The SL_API is a platform-specific macro used
* to declare OPENSL ES function prototypes. It is modified to meet the
* requirements for a particular platform
*
* Example:
* #ifdef __SYMBIAN32__
* # define SL_API __declspec(dllimport)
* #endif
*/
#ifndef SL_API
#define SL_API                      /* override per-platform */
#endif

#endif /* _OPENSLES_PLATFORM_H_ */
